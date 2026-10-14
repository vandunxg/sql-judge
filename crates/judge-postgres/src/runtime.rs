use super::{ManagedPostgresSession, PostgresSession, QueryExecutionError};
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct ManagedPostgresRuntime {
    root: PathBuf,
    bin: PathBuf,
    connection: String,
    stop_on_drop: bool,
}

impl ManagedPostgresRuntime {
    pub fn adopt(root: impl AsRef<Path>, instance_id: &str) -> Result<Self, QueryExecutionError> {
        if instance_id.trim().is_empty() {
            return Err(QueryExecutionError::PolicyViolation);
        }

        let root = root.as_ref().to_path_buf();
        if !root.exists() {
            return Err(QueryExecutionError::ConnectionError);
        }
        if !root.is_absolute() || !root.join("PG_VERSION").is_file() {
            return Err(QueryExecutionError::PolicyViolation);
        }
        let user = os_user()?;
        let bin =
            postgres_bin_dir(cluster_major(&root)).ok_or(QueryExecutionError::ConnectionError)?;
        // A stopped instance is merely unavailable; its socket directory may already be gone.
        if !is_running(&bin, &root)? {
            return Err(QueryExecutionError::ConnectionError);
        }
        let endpoint = platform::Endpoint::existing(&root)?;

        let connection = endpoint.connection_string(&user);
        ManagedPostgresSession::connect(&connection, instance_id)?;
        Ok(Self {
            root,
            bin,
            connection,
            stop_on_drop: true,
        })
    }

    pub fn start(root: impl AsRef<Path>, instance_id: &str) -> Result<Self, QueryExecutionError> {
        if instance_id.trim().is_empty() {
            return Err(QueryExecutionError::PolicyViolation);
        }

        let root = root.as_ref().to_path_buf();
        if !root.is_absolute() {
            return Err(QueryExecutionError::PolicyViolation);
        }
        // A start that failed before `initdb` ran leaves an empty app-owned directory behind.
        let fresh = match fs::read_dir(&root) {
            Ok(mut entries) => entries.next().is_none(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(_) => return Err(QueryExecutionError::ConnectionError),
        };
        if !fresh && !root.join("PG_VERSION").is_file() {
            return Err(QueryExecutionError::PolicyViolation);
        }
        let user = os_user()?;
        let bin =
            postgres_bin_dir(cluster_major(&root)).ok_or(QueryExecutionError::ConnectionError)?;

        if fresh {
            if !root.exists() {
                let parent = root.parent().ok_or(QueryExecutionError::PolicyViolation)?;
                fs::create_dir_all(parent).map_err(|_| QueryExecutionError::ConnectionError)?;
                // `create_dir` claims the root, so a second first launch backs off instead of
                // initializing (and later removing) the same directory.
                fs::create_dir(&root).map_err(|_| QueryExecutionError::ConnectionError)?;
            }
            // A failed initdb removes what it wrote, leaving the root empty and fresh again.
            platform::initialize(&bin, &root, &user)?;
        }

        let running = is_running(&bin, &root)?;
        let endpoint = if running {
            platform::Endpoint::existing(&root)?
        } else {
            platform::Endpoint::prepare(&root)?
        };
        if !running {
            let mut start = tool(&bin, "pg_ctl");
            start.arg("-D").arg(&root);
            if let Some(options) = endpoint.server_options()? {
                start.args(["-o", &options]);
            }
            let started = start
                .args(["-w", "start"])
                .status()
                .map_err(|_| QueryExecutionError::ConnectionError)?;
            if !started.success() {
                // Another launch may have started this fresh cluster in the meantime.
                if fresh && !is_running(&bin, &root)? {
                    let _ = fs::remove_dir_all(&root);
                }
                return Err(QueryExecutionError::ConnectionError);
            }
        }

        let mut runtime = Self {
            root,
            bin,
            connection: endpoint.connection_string(&user),
            stop_on_drop: !running,
        };
        let initialized = if fresh {
            runtime.initialize(instance_id)
        } else {
            ManagedPostgresSession::connect(&runtime.connection, instance_id).map(|_| ())
        };
        if let Err(error) = initialized {
            drop(runtime);
            return Err(error);
        }
        runtime.stop_on_drop = true;
        Ok(runtime)
    }

    pub fn connection_string(&self) -> String {
        self.connection.clone()
    }

    fn initialize(&self, instance_id: &str) -> Result<(), QueryExecutionError> {
        let mut session = PostgresSession::connect(&self.connection)?;
        session
            .client
            .batch_execute(
                "CREATE ROLE sqljudge_runner NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOBYPASSRLS NOREPLICATION NOLOGIN;
                 REVOKE CREATE ON SCHEMA public FROM PUBLIC;
                 CREATE TABLE public.__sql_judge_runtime (instance_id text PRIMARY KEY)",
            )
            .map_err(|_| QueryExecutionError::RuntimeError)?;
        session
            .client
            .execute(
                "INSERT INTO public.__sql_judge_runtime (instance_id) VALUES ($1)",
                &[&instance_id],
            )
            .map_err(|_| QueryExecutionError::RuntimeError)?;
        session
            .client
            .batch_execute("REVOKE ALL ON TABLE public.__sql_judge_runtime FROM PUBLIC")
            .map_err(|_| QueryExecutionError::RuntimeError)
    }
}

impl Drop for ManagedPostgresRuntime {
    fn drop(&mut self) {
        if !self.stop_on_drop {
            return;
        }
        let _ = tool(&self.bin, "pg_ctl")
            .arg("-D")
            .arg(&self.root)
            .args(["-w", "stop", "-m", "fast"])
            .status();
    }
}

fn os_user() -> Result<String, QueryExecutionError> {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .map_err(|_| QueryExecutionError::ConnectionError)?;
    if user.trim().is_empty() {
        return Err(QueryExecutionError::ConnectionError);
    }
    Ok(user)
}

fn is_running(bin: &Path, root: &Path) -> Result<bool, QueryExecutionError> {
    Ok(tool(bin, "pg_ctl")
        .arg("-D")
        .arg(root)
        .arg("status")
        .status()
        .is_ok_and(|status| status.success()))
}

fn tool(bin: &Path, name: &str) -> Command {
    let mut command = Command::new(bin.join(format!("{name}{EXE_SUFFIX}")));
    command.stdout(Stdio::null()).stderr(Stdio::null());
    // Without LC_ALL (every Finder launch) macOS starts locale threads and the postmaster
    // aborts with "postmaster became multithreaded during startup".
    #[cfg(target_os = "macos")]
    command.env("LC_ALL", "C");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// GUI apps do not inherit the shell PATH (Finder, Start menu), and common installers
/// (keg-only Homebrew, Postgres.app, EDB, Debian) never add the server tools to it.
/// An existing cluster only starts under the major version that created it.
fn postgres_bin_dir(cluster_major: Option<u32>) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .filter(|dir| dir.is_absolute())
        .chain(installed_bin_dirs())
        .filter(|dir| {
            ["initdb", "pg_ctl", "postgres"]
                .iter()
                .all(|name| dir.join(format!("{name}{EXE_SUFFIX}")).is_file())
        })
        .find(|dir| cluster_major.is_none() || tools_major(dir) == cluster_major)
}

fn cluster_major(root: &Path) -> Option<u32> {
    fs::read_to_string(root.join("PG_VERSION"))
        .ok()
        .and_then(|version| leading_number(&version))
}

fn tools_major(bin: &Path) -> Option<u32> {
    let mut command = tool(bin, "pg_ctl");
    let output = command
        .arg("--version")
        .stdout(Stdio::piped())
        .output()
        .ok()?;
    version_output_major(&String::from_utf8_lossy(&output.stdout))
}

/// `pg_ctl (PostgreSQL) 17.2 (Homebrew)`; packagers append their own suffix.
fn version_output_major(output: &str) -> Option<u32> {
    leading_number(output.split_once("(PostgreSQL)")?.1)
}

fn leading_number(text: &str) -> Option<u32> {
    text.trim_start_matches(|c: char| !c.is_ascii_digit())
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

fn installed_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(target_os = "macos")]
    {
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
        dirs.extend(versioned_bin_dirs(
            Path::new("/opt/homebrew/opt"),
            "postgresql",
        ));
        dirs.extend(versioned_bin_dirs(
            Path::new("/usr/local/opt"),
            "postgresql",
        ));
        dirs.extend(versioned_bin_dirs(
            Path::new("/Applications/Postgres.app/Contents/Versions"),
            "",
        ));
        dirs.extend(versioned_bin_dirs(
            Path::new("/opt/local/lib"),
            "postgresql",
        ));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs.extend(versioned_bin_dirs(Path::new("/usr/lib/postgresql"), ""));
        dirs.extend(versioned_bin_dirs(Path::new("/usr"), "pgsql-"));
    }
    #[cfg(windows)]
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        dirs.extend(versioned_bin_dirs(
            &Path::new(&program_files).join("PostgreSQL"),
            "",
        ));
    }
    dirs
}

/// `<parent>/<prefix><major>…/bin` for every install under `parent`, newest major first.
fn versioned_bin_dirs(parent: &Path, prefix: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut found: Vec<(u32, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let major = leading_number(name.strip_prefix(prefix)?).unwrap_or(0);
            Some((major, entry.path().join("bin")))
        })
        .collect();
    found.sort_by_key(|(major, _)| std::cmp::Reverse(*major));
    found.into_iter().map(|(_, dir)| dir).collect()
}

/// Quotes a libpq keyword/value connection parameter.
fn connection_value(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// FNV-1a: a stable short name derived from the runtime root.
fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(unix)]
mod platform {
    use super::{QueryExecutionError, connection_value, stable_hash, tool};
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};

    const SOCKET_FILE: &str = ".s.PGSQL.5432";
    /// `sockaddr_un.sun_path` including the terminating NUL: 108 bytes on Linux, 104 on macOS.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const MAX_SOCKET_PATH_BYTES: usize = 107;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const MAX_SOCKET_PATH_BYTES: usize = 103;

    pub(super) struct Endpoint {
        socket: PathBuf,
    }

    pub(super) fn initialize(
        bin: &Path,
        root: &Path,
        user: &str,
    ) -> Result<(), QueryExecutionError> {
        let initialized = tool(bin, "initdb")
            .arg("-D")
            .arg(root)
            .args([
                "--username",
                user,
                "--auth-local=peer",
                "--auth-host=reject",
                // Finder-launched apps have no LANG, which would otherwise yield SQL_ASCII.
                "--encoding=UTF8",
                "--locale=C",
            ])
            .status()
            .map_err(|_| QueryExecutionError::ConnectionError)?;
        if initialized.success() {
            Ok(())
        } else {
            Err(QueryExecutionError::ConnectionError)
        }
    }

    impl Endpoint {
        pub(super) fn existing(root: &Path) -> Result<Self, QueryExecutionError> {
            let socket = socket_dir(root);
            verify_private_dir(&socket, root)?;
            Ok(Self { socket })
        }

        pub(super) fn prepare(root: &Path) -> Result<Self, QueryExecutionError> {
            let socket = socket_dir(root);
            ensure_private_dir(&socket, root)?;
            Ok(Self { socket })
        }

        pub(super) fn server_options(&self) -> Result<Option<String>, QueryExecutionError> {
            let socket = self
                .socket
                .to_str()
                .ok_or(QueryExecutionError::PolicyViolation)?;
            // pg_ctl hands `-o` to /bin/sh; postgres splits the directory list on commas.
            if socket.contains(['\'', ',']) {
                return Err(QueryExecutionError::PolicyViolation);
            }
            Ok(Some(format!(
                "-k '{socket}' -p 5432 -c listen_addresses=''"
            )))
        }

        pub(super) fn connection_string(&self, user: &str) -> String {
            format!(
                "host={} user={} dbname=postgres",
                connection_value(&self.socket.to_string_lossy()),
                connection_value(user)
            )
        }
    }

    fn socket_dir(root: &Path) -> PathBuf {
        let preferred = root.join("socket");
        if preferred.join(SOCKET_FILE).as_os_str().len() <= MAX_SOCKET_PATH_BYTES {
            return preferred;
        }
        std::env::temp_dir().join(format!(
            "sqljudge-{:016x}",
            stable_hash(root.as_os_str().as_encoded_bytes())
        ))
    }

    fn ensure_private_dir(path: &Path, owner_of: &Path) -> Result<(), QueryExecutionError> {
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                verify_private_dir(path, owner_of)
            }
            Err(_) => Err(QueryExecutionError::ConnectionError),
        }
    }

    /// A socket directory outside the root may be planted by another local user.
    fn verify_private_dir(path: &Path, owner_of: &Path) -> Result<(), QueryExecutionError> {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| QueryExecutionError::ConnectionError)?;
        let owner = fs::metadata(owner_of)
            .map_err(|_| QueryExecutionError::ConnectionError)?
            .uid();
        if !metadata.is_dir() || metadata.uid() != owner {
            return Err(QueryExecutionError::PolicyViolation);
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| QueryExecutionError::PolicyViolation)?;
        }
        Ok(())
    }
}

/// Windows has no peer authentication and rust-postgres has no Unix sockets there, so the
/// runtime listens on loopback TCP with a random superuser password kept in the Windows
/// Credential Manager (ADR-013 amendment).
#[cfg(windows)]
mod platform {
    use super::{QueryExecutionError, connection_value, stable_hash, tool};
    use keyring_core::Entry;
    use keyring_core::api::CredentialStoreApi;
    use std::fs;
    use std::io::Write;
    use std::net::TcpListener;
    use std::path::Path;

    const CREDENTIAL_SERVICE: &str = "sql-judge-managed-runtime";

    pub(super) struct Endpoint {
        port: u16,
        password: String,
    }

    pub(super) fn initialize(
        bin: &Path,
        root: &Path,
        user: &str,
    ) -> Result<(), QueryExecutionError> {
        // Reused when present and stored before initdb, so neither a crash after initdb nor a
        // concurrent first launch can leave the cluster with a password nobody holds.
        let credential = credential(root)?;
        let password = match credential.get_password() {
            Ok(password) => password,
            Err(keyring_core::Error::NoEntry) => {
                let password = format!(
                    "{}{}",
                    uuid::Uuid::new_v4().simple(),
                    uuid::Uuid::new_v4().simple()
                );
                credential
                    .set_password(&password)
                    .map_err(|_| QueryExecutionError::ConnectionError)?;
                password
            }
            Err(_) => return Err(QueryExecutionError::ConnectionError),
        };
        let parent = root.parent().ok_or(QueryExecutionError::PolicyViolation)?;
        let password_file =
            parent.join(format!(".sqljudge-init-{}", uuid::Uuid::new_v4().simple()));
        let written = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&password_file)
            .and_then(|mut file| file.write_all(password.as_bytes()));
        let initialized = written.ok().and_then(|()| {
            tool(bin, "initdb")
                .arg("-D")
                .arg(root)
                .args([
                    "--username",
                    user,
                    "--auth=scram-sha-256",
                    "--encoding=UTF8",
                    "--locale=C",
                ])
                .arg(format!("--pwfile={}", password_file.display()))
                .status()
                .ok()
        });
        let _ = fs::remove_file(&password_file);
        if initialized.is_some_and(|status| status.success()) {
            Ok(())
        } else {
            Err(QueryExecutionError::ConnectionError)
        }
    }

    impl Endpoint {
        /// The running postmaster records its port on the fourth line of `postmaster.pid`.
        pub(super) fn existing(root: &Path) -> Result<Self, QueryExecutionError> {
            let pid_file = fs::read_to_string(root.join("postmaster.pid"))
                .map_err(|_| QueryExecutionError::ConnectionError)?;
            let port = pid_file
                .lines()
                .nth(3)
                .and_then(|port| port.trim().parse().ok())
                .ok_or(QueryExecutionError::PolicyViolation)?;
            Ok(Self {
                port,
                password: password(root)?,
            })
        }

        /// A port picked per start cannot be lost to another program between launches.
        pub(super) fn prepare(root: &Path) -> Result<Self, QueryExecutionError> {
            let port = TcpListener::bind(("127.0.0.1", 0))
                .and_then(|listener| listener.local_addr())
                .map_err(|_| QueryExecutionError::ConnectionError)?
                .port();
            Ok(Self {
                port,
                password: password(root)?,
            })
        }

        pub(super) fn server_options(&self) -> Result<Option<String>, QueryExecutionError> {
            Ok(Some(format!(
                "-c listen_addresses=127.0.0.1 -p {}",
                self.port
            )))
        }

        pub(super) fn connection_string(&self, user: &str) -> String {
            format!(
                "host=127.0.0.1 port={} user={} password={} dbname=postgres",
                self.port,
                connection_value(user),
                connection_value(&self.password)
            )
        }
    }

    fn password(root: &Path) -> Result<String, QueryExecutionError> {
        credential(root)?
            .get_password()
            .map_err(|_| QueryExecutionError::ConnectionError)
    }

    /// Built straight from the Windows store so the app-wide default keyring store is untouched.
    fn credential(root: &Path) -> Result<Entry, QueryExecutionError> {
        windows_native_keyring_store::Store::new()
            .and_then(|store| {
                store.build(
                    CREDENTIAL_SERVICE,
                    &format!("{:016x}", stable_hash(root.as_os_str().as_encoded_bytes())),
                    None,
                )
            })
            .map_err(|_| QueryExecutionError::ConnectionError)
    }
}

#[cfg(test)]
mod tests {
    use super::{connection_value, leading_number, version_output_major, versioned_bin_dirs};
    use std::fs;

    #[test]
    fn versioned_bin_dirs_prefer_the_newest_major_and_skip_other_entries() {
        let parent = std::env::temp_dir().join(format!(
            "sql-judge-bin-dirs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        for name in ["postgresql@9", "postgresql@17", "postgresql@16", "redis"] {
            fs::create_dir_all(parent.join(name)).expect("create install directory");
        }

        let dirs = versioned_bin_dirs(&parent, "postgresql");

        let expected: Vec<_> = ["postgresql@17", "postgresql@16", "postgresql@9"]
            .map(|name| parent.join(name).join("bin"))
            .into();
        assert_eq!(dirs, expected);
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn major_versions_parse_from_pg_version_and_tool_output() {
        assert_eq!(leading_number("17\n"), Some(17));
        assert_eq!(leading_number("18.1"), Some(18));
        assert_eq!(leading_number("@16"), Some(16));
        assert_eq!(leading_number("latest"), None);
        for (output, major) in [
            ("pg_ctl (PostgreSQL) 17.2 (Homebrew)\n", 17),
            (
                "pg_ctl (PostgreSQL) 18.6 (Ubuntu 18.6-0ubuntu0.26.04.1)\n",
                18,
            ),
            ("pg_ctl (PostgreSQL) 16.4\r\n", 16),
        ] {
            assert_eq!(version_output_major(output), Some(major));
        }
        assert_eq!(version_output_major("pg_ctl 9000"), None);
    }

    #[test]
    fn connection_values_are_quoted_against_whitespace_and_injection() {
        assert_eq!(
            connection_value("/Users/a b/Application Support"),
            "'/Users/a b/Application Support'"
        );
        assert_eq!(connection_value(r"it's\x"), r"'it\'s\\x'");
    }
}
