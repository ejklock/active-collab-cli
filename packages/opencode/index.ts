import { fileURLToPath } from "node:url"

type McpEntry = { type: "local"; command: string[] } & Record<string, unknown>
type OpencodeConfig = {
  mcp?: Record<string, McpEntry>
  skills?: { paths?: string[] }
}

const skillsDir = fileURLToPath(new URL("./skills", import.meta.url))

// OpenCode 1.19 can register an MCP server and a skills directory only by
// mutating the merged config in the `config` hook. A user's own
// `active-collab` entry wins over the default, and the skills directory is
// listed once however often the hook runs.
export const server = async () => ({
  config: async (config: OpencodeConfig): Promise<void> => {
    config.mcp = { "active-collab": { type: "local", command: ["ac", "mcp"] }, ...config.mcp }
    const paths = config.skills?.paths ?? []
    config.skills = { ...config.skills, paths: paths.includes(skillsDir) ? paths : [...paths, skillsDir] }
  },
})
