import { describe, expect, test } from "bun:test"

import {
  draftForEngine,
  draftFromProfile,
  draftFromUrl,
  saveRequestFromDraft,
  tlsModeLabel,
  tlsShortLabel,
} from "./connections-profile"

const profile = {
  profileId: "hr",
  displayName: "Staging HR",
  engine: "sqlserver",
  host: "sql-stg",
  port: 1433,
  database: "hr",
  username: "reader",
  tlsMode: "verify_ca",
  hasSecret: true,
}

describe("connection profile drafts", () => {
  test("loads a saved profile without any secret and with a supported TLS mode", () => {
    expect(draftFromProfile(profile)).toEqual({
      profileId: "hr",
      displayName: "Staging HR",
      engine: "sqlserver",
      host: "sql-stg",
      port: 1433,
      database: "hr",
      username: "reader",
      tlsMode: "verify_full",
      secret: "",
    })
  })

  test("omits a blank secret from the save request so the vault entry is kept", () => {
    const draft = draftFromProfile(profile)
    expect({
      blank: "secret" in saveRequestFromDraft({ ...draft, secret: "   " }),
      provided: saveRequestFromDraft({ ...draft, secret: "s3cret" }).secret,
    }).toEqual({ blank: false, provided: "s3cret" })
  })

  test("switching engine resets the default port and normalizes TLS", () => {
    const draft = { ...draftFromProfile(profile), engine: "postgresql", port: 5432, tlsMode: "verify_ca" }
    expect(draftForEngine(draft, "sqlserver")).toMatchObject({
      engine: "sqlserver",
      port: 1433,
      tlsMode: "verify_full",
    })
    expect(draftForEngine(draft, "postgresql")).toMatchObject({ port: 5432, tlsMode: "verify_ca" })
  })

  test("labels TLS modes without overstating verification", () => {
    expect(["verify_full", "verify_ca", "require"].map((mode) => [tlsModeLabel(mode), tlsShortLabel(mode)])).toEqual([
      ["Verify certificate and host", "TLS verified"],
      ["Verify certificate authority", "TLS CA verified"],
      ["Require TLS (no certificate check)", "TLS not verified"],
    ])
  })
})

describe("connection URL import", () => {
  test("fills parsed fields and suggests names only where the draft is blank", () => {
    const parsed = {
      engine: "postgresql",
      host: "db.internal",
      port: 5433,
      database: "analytics",
      username: "reader",
      secret: "pw",
      tlsMode: "verify_ca",
    }
    const blank = draftFromUrl({ ...draftFromProfile(profile), profileId: "", displayName: "" }, parsed)
    const named = draftFromUrl(draftFromProfile(profile), parsed)
    expect({
      blank: [blank.profileId.length > 0, blank.displayName.length > 0, blank.host, blank.port, blank.secret],
      named: [named.profileId, named.displayName, named.engine, named.tlsMode],
    }).toEqual({
      blank: [true, true, "db.internal", 5433, "pw"],
      named: ["hr", "Staging HR", "postgresql", "verify_ca"],
    })
  })
})
