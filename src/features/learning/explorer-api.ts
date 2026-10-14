import {
  executeExternalSelect,
  executeExternalSqlServerSelect,
  listExternalMetadata,
  listExternalSqlServerMetadata,
  testExternalConnection,
  testExternalSqlServer,
  type ConnectionProfileSummary,
} from "@/lib/judge-api"

export const EXPLORER_LIMITS = {
  statementTimeoutMs: 3_000,
  maxRows: 100,
  maxBytes: 256 * 1024,
}

const COMMANDS = {
  postgresql: {
    test: testExternalConnection,
    metadata: listExternalMetadata,
    select: executeExternalSelect,
  },
  sqlserver: {
    test: testExternalSqlServer,
    metadata: listExternalSqlServerMetadata,
    select: executeExternalSqlServerSelect,
  },
}

export function externalCommandsFor(engine: ConnectionProfileSummary["engine"]) {
  return COMMANDS[engine]
}
