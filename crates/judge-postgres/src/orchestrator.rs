use crate::{
    CancellationReason, CancellationToken, ManagedEnvironment, PostgresSession, QueryExecutionError,
};
use judge_core::{
    ComparisonMismatch, OutputPolicy, QueryLimits, QueryResult, Verdict, aggregate_verdicts,
    compare_query_results,
};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct JudgeCase<'a> {
    pub id: &'a str,
    pub schema_sql: &'a str,
    pub fixture_sql: &'a str,
    pub reference_sql: &'a str,
    pub output: OutputPolicy,
    pub limits: QueryLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestOutcome {
    pub id: String,
    pub verdict: Verdict,
    pub mismatch: Option<ComparisonMismatch>,
    /// Wall-clock time of the user's query for this fixture; 0 when it did not run.
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionReport {
    pub verdict: Verdict,
    pub tests: Vec<TestOutcome>,
    pub preview: Option<QueryResult>,
}

/// Where each case's fixture is loaded. Both targets roll every fixture back.
#[derive(Clone, Copy)]
enum FixtureTarget<'a> {
    /// A disposable schema built from the case schema, used for Submit.
    Disposable,
    /// The pack's provisioned sandbox namespace, used for Run (ADR-026).
    Namespace(&'a ManagedEnvironment),
}

struct FixtureExecution {
    reference: Result<QueryResult, QueryExecutionError>,
    user: Option<Result<QueryResult, QueryExecutionError>>,
    user_elapsed_ms: u64,
}

pub fn judge_submission(
    session: &mut PostgresSession,
    runner_role: &str,
    cases: &[JudgeCase<'_>],
    user_sql: &str,
) -> SubmissionReport {
    judge_submission_inner(
        session,
        FixtureTarget::Disposable,
        runner_role,
        cases,
        user_sql,
        &|| false,
        None,
    )
}

pub fn judge_submission_with_cancel<F>(
    session: &mut PostgresSession,
    runner_role: &str,
    cases: &[JudgeCase<'_>],
    user_sql: &str,
    is_cancelled: F,
) -> SubmissionReport
where
    F: Fn() -> bool,
{
    judge_submission_inner(
        session,
        FixtureTarget::Disposable,
        runner_role,
        cases,
        user_sql,
        &is_cancelled,
        None,
    )
}

pub fn judge_submission_with_token(
    session: &mut PostgresSession,
    runner_role: &str,
    cases: &[JudgeCase<'_>],
    user_sql: &str,
    token: &CancellationToken,
) -> SubmissionReport {
    judge_submission_inner(
        session,
        FixtureTarget::Disposable,
        runner_role,
        cases,
        user_sql,
        &|| token.is_cancelled(),
        Some(token),
    )
}

pub fn judge_submission_with_watchdog(
    session: &mut PostgresSession,
    runner_role: &str,
    cases: &[JudgeCase<'_>],
    user_sql: &str,
    watchdog_after: Duration,
) -> SubmissionReport {
    let token = CancellationToken::default();
    judge_submission_with_watchdog_token(
        session,
        runner_role,
        cases,
        user_sql,
        &token,
        watchdog_after,
    )
}

pub fn judge_submission_with_watchdog_token(
    session: &mut PostgresSession,
    runner_role: &str,
    cases: &[JudgeCase<'_>],
    user_sql: &str,
    token: &CancellationToken,
    watchdog_after: Duration,
) -> SubmissionReport {
    let (finished, deadline) = mpsc::channel();
    let watchdog_token = token.clone();
    let watchdog = thread::spawn(move || {
        if deadline.recv_timeout(watchdog_after).is_err() {
            watchdog_token.cancel_for_watchdog();
        }
    });
    let report = judge_submission_with_token(session, runner_role, cases, user_sql, token);
    let _ = finished.send(());
    let _ = watchdog.join();
    report
}

/// Judges each case against the pack sandbox namespace instead of a disposable schema. The
/// cases' `schema_sql` is ignored: the namespace was provisioned with the pack schema.
pub fn judge_submission_in_namespace_with_watchdog_token(
    session: &mut PostgresSession,
    environment: &ManagedEnvironment,
    runner_role: &str,
    cases: &[JudgeCase<'_>],
    user_sql: &str,
    token: &CancellationToken,
    watchdog_after: Duration,
) -> SubmissionReport {
    let (finished, deadline) = mpsc::channel();
    let watchdog_token = token.clone();
    let watchdog = thread::spawn(move || {
        if deadline.recv_timeout(watchdog_after).is_err() {
            watchdog_token.cancel_for_watchdog();
        }
    });
    let report = judge_submission_inner(
        session,
        FixtureTarget::Namespace(environment),
        runner_role,
        cases,
        user_sql,
        &|| token.is_cancelled(),
        Some(token),
    );
    let _ = finished.send(());
    let _ = watchdog.join();
    report
}

fn judge_submission_inner(
    session: &mut PostgresSession,
    target: FixtureTarget<'_>,
    runner_role: &str,
    cases: &[JudgeCase<'_>],
    user_sql: &str,
    is_cancelled: &dyn Fn() -> bool,
    cancellation: Option<&CancellationToken>,
) -> SubmissionReport {
    let mut tests = Vec::with_capacity(cases.len());
    let mut preview = None;

    for case in cases {
        if is_cancelled() {
            return SubmissionReport {
                verdict: cancellation_verdict(cancellation),
                tests,
                preview,
            };
        }
        let run_case = |session: &mut PostgresSession| {
            let reference = match cancellation {
                Some(token) => session.execute_with_cancel(case.reference_sql, case.limits, token),
                None => session.execute(case.reference_sql, case.limits),
            };
            let user_started = Instant::now();
            let user = reference.as_ref().ok().map(|_| match cancellation {
                Some(token) => session.execute_with_cancel(user_sql, case.limits, token),
                None => session.execute(user_sql, case.limits),
            });
            let user_elapsed_ms = if user.is_some() {
                u64::try_from(user_started.elapsed().as_millis()).unwrap_or(u64::MAX)
            } else {
                0
            };
            Ok(FixtureExecution {
                reference,
                user,
                user_elapsed_ms,
            })
        };
        let execution = match target {
            FixtureTarget::Disposable => session.with_managed_fixture(
                runner_role,
                case.schema_sql,
                case.fixture_sql,
                run_case,
            ),
            FixtureTarget::Namespace(environment) => {
                session.with_namespace_fixture(environment, runner_role, case.fixture_sql, run_case)
            }
        };

        if is_cancelled() {
            return SubmissionReport {
                verdict: cancellation_verdict(cancellation),
                tests,
                preview,
            };
        }

        let elapsed_ms = execution
            .as_ref()
            .map_or(0, |execution| execution.user_elapsed_ms);
        let outcome = match execution {
            Err(_) => TestOutcome {
                id: case.id.to_owned(),
                verdict: Verdict::JudgeError,
                mismatch: None,
                elapsed_ms,
            },
            Ok(FixtureExecution {
                reference: Err(_), ..
            }) => TestOutcome {
                id: case.id.to_owned(),
                verdict: Verdict::JudgeError,
                mismatch: None,
                elapsed_ms,
            },
            Ok(FixtureExecution {
                reference: Ok(_),
                user: Some(Err(error)),
                ..
            }) => TestOutcome {
                id: case.id.to_owned(),
                verdict: user_error_verdict(error),
                mismatch: None,
                elapsed_ms,
            },
            Ok(FixtureExecution {
                reference: Ok(expected),
                user: Some(Ok(actual)),
                ..
            }) => {
                if preview.is_none() {
                    preview = Some(actual.clone());
                }
                let comparison = compare_query_results(&expected, &actual, &case.output);
                TestOutcome {
                    id: case.id.to_owned(),
                    verdict: if comparison.is_match() {
                        Verdict::Accepted
                    } else {
                        Verdict::WrongAnswer
                    },
                    mismatch: comparison.mismatch,
                    elapsed_ms,
                }
            }
            Ok(FixtureExecution {
                reference: Ok(_),
                user: None,
                ..
            }) => TestOutcome {
                id: case.id.to_owned(),
                verdict: Verdict::JudgeError,
                mismatch: None,
                elapsed_ms,
            },
        };
        tests.push(outcome);
    }

    let verdicts: Vec<_> = tests.iter().map(|test| test.verdict).collect();
    SubmissionReport {
        verdict: aggregate_verdicts(&verdicts),
        tests,
        preview,
    }
}

fn cancellation_verdict(token: Option<&CancellationToken>) -> Verdict {
    match token.and_then(CancellationToken::reason) {
        Some(CancellationReason::Watchdog) => Verdict::TimeLimitExceeded,
        Some(CancellationReason::User) | None => Verdict::Cancelled,
    }
}

fn user_error_verdict(error: QueryExecutionError) -> Verdict {
    match error {
        QueryExecutionError::PolicyViolation => Verdict::PolicyViolation,
        QueryExecutionError::SyntaxError => Verdict::SyntaxError,
        QueryExecutionError::TimeLimitExceeded => Verdict::TimeLimitExceeded,
        QueryExecutionError::Cancelled => Verdict::Cancelled,
        QueryExecutionError::ResultLimitExceeded => Verdict::ResultLimitExceeded,
        QueryExecutionError::RuntimeError => Verdict::RuntimeError,
        QueryExecutionError::InvalidLimits(_)
        | QueryExecutionError::UnsupportedType(_)
        | QueryExecutionError::ConnectionError
        | QueryExecutionError::OwnershipVerificationFailed => Verdict::JudgeError,
    }
}
