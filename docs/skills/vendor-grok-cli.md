https://docs.x.ai/build/features/skills-plugins-marketplaces#skills
https://docs.x.ai/build/features/project-rules
https://docs.x.ai/build/features/mcp-servers

Skills
Skills are reusable folders containing markdown instructions, script files, and resources for agents.

Grok discovers skills from:

./.grok/skills/ (walked up to the repo root)
~/.grok/skills/
Any enabled plugin's skills/ directory
Extra paths under [skills] paths in ~/.grok/config.toml
User-invocable skills also appear as slash commands, for example /<skill-name>.

Plugins
Plugins extend Grok with additional skills, agents, hooks, MCP servers, and LSP servers.

Grok loads plugins from:

./.grok/plugins/
~/.grok/plugins/
Marketplace installs under ~/.grok/plugins/marketplaces/
Extra paths under [plugins] paths in ~/.grok/config.toml
--plugin-dir <PATH> on the CLI
Manage plugins, hooks, skills, and MCP servers from a single extensions modal in the TUI — open it with any of /plugins, /hooks, /skills, or /mcps.

AGENTS.md

Copy for LLM
View as Markdown
Manage API keys
Meet grok-4.5
Project rules are Markdown files that Grok loads into context for every session in a directory tree. Put coding conventions, build and test commands, and architecture notes in an AGENTS.md at your repo root, and Grok follows them without being told each session.

Discovery
Grok loads rules in this order, with deeper files taking precedence on conflicts:

Global rules in ~/.grok/
Every directory from the repo root down to the working directory (or only the working directory outside a git repo)
Within each directory, Grok reads any of AGENTS.md, Agents.md, AGENT.md, CLAUDE.md, Claude.md, and CLAUDE.local.md, plus every *.md file in a .grok/rules/ directory (.claude/rules/ and .cursor/rules/ are read for compatibility). Files ignored by .gitignore are skipped, which keeps personal overrides like CLAUDE.local.md out of shared context.

A nested AGENTS.md scopes to its subtree, so a monorepo can carry different conventions per package:

Text


my-monorepo/
  AGENTS.md                # repo-wide rules
  packages/
    frontend/AGENTS.md     # "Use React. Prefer CSS modules."
    backend/AGENTS.md      # "Use Express. Follow REST conventions."
Files are loaded in full, with no size cap; short, specific instructions are followed more reliably than long ones.

Session rules
To add rules for a single run without editing files, pass --rules (Grok appends the text to the system prompt), or --system-prompt-override to replace the system prompt entirely:

Bash


grok --rules "Always use TypeScript. Prefer functional components."
Verification
Bash


grok inspect
This lists each rules file Grok found, with its path and approximate token count.

Features
MCP Servers

Copy for LLM
View as Markdown
Manage API keys
Meet grok-4.5
MCP (Model Context Protocol) servers expose external tools to Grok. Once configured, their tools are available alongside the built-in ones, namespaced as <server>__<tool>.

Adding a server
The fastest way is the grok mcp command:

Bash


# Local stdio server; everything after -- is the server command
grok mcp add filesystem -- npx -y @modelcontextprotocol/server-filesystem /path/to/dir
# Remote server over HTTP (OAuth handled automatically)
grok mcp add --transport http linear https://mcp.linear.app/mcp
# Remote server with a static auth header (--header is repeatable)
grok mcp add --transport http api https://mcp.example.com/mcp --header "Authorization: Bearer ${API_TOKEN}"
grok mcp list shows configured servers, grok mcp remove <name> deletes one, and grok mcp doctor [name] diagnoses configuration and connectivity. list and doctor take --json for machine-readable output.

Servers can also be declared directly in ~/.grok/config.toml:

TOML


[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"]
env = { API_KEY = "${MY_API_KEY}" }   # ${VAR} expands at load time
startup_timeout_sec = 30              # default 30
tool_timeout_sec = 6000               # default 6000
[mcp_servers.linear]
url = "https://mcp.linear.app/mcp"
headers = { "x-mcp-session-id" = "{{session_id}}" }
Grok expands ${VAR} (and ${VAR:-default}) in url, command, args, env, and headers, so secrets can stay in the environment. Servers that require OAuth trigger a browser flow on first use; tokens are stored under ~/.grok/mcp_credentials.json.

Project scope
Pass --scope project to grok mcp add (it writes .grok/config.toml in the current directory) to define servers that ship with the repo. When loading, Grok walks from the current directory up to the git root reading each .grok/config.toml, and a project server with the same name as a user one replaces it entirely.

In the TUI
/mcps opens the MCP tab of the extensions modal: toggle a server with Space, refresh after config edits with r, authenticate OAuth servers with i, and add or remove with a and x.

Compatibility
Grok also loads MCP configurations from ~/.claude.json, .cursor/mcp.json, and project .mcp.json files, merged below config.toml in priority. Disable a vendor with [compat.claude] mcps = false or [compat.cursor] mcps = false. grok inspect shows every loaded server and its origin.

Troubleshooting
grok mcp doctor is the first stop. For stdio servers that start but fail to connect, Grok captures stderr to ~/.grok/logs/mcp/<server>.stderr.log. Cold-start npx servers that download packages on first launch may need a higher startup_timeout_sec.


