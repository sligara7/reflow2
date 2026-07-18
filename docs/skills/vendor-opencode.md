https://opencode.ai/docs/skills/

Agent Skills
Define reusable behavior via SKILL.md definitions

Agent skills let OpenCode discover reusable instructions from your repo or home directory. Skills are loaded on-demand via the native skill tool—agents see available skills and can load the full content when needed.

Place files
Create one folder per skill name and put a SKILL.md inside it. OpenCode searches these locations:

Project config: .opencode/skills/<name>/SKILL.md
Global config: ~/.config/opencode/skills/<name>/SKILL.md
Project Claude-compatible: .claude/skills/<name>/SKILL.md
Global Claude-compatible: ~/.claude/skills/<name>/SKILL.md
Project agent-compatible: .agents/skills/<name>/SKILL.md
Global agent-compatible: ~/.agents/skills/<name>/SKILL.md
Understand discovery
For project-local paths, OpenCode walks up from your current working directory until it reaches the git worktree. It loads any matching skills/*/SKILL.md in .opencode/ and any matching .claude/skills/*/SKILL.md or .agents/skills/*/SKILL.md along the way.

Global definitions are also loaded from ~/.config/opencode/skills/*/SKILL.md, ~/.claude/skills/*/SKILL.md, and ~/.agents/skills/*/SKILL.md.

Write frontmatter
Each SKILL.md must start with YAML frontmatter. Only these fields are recognized:

name (required)
description (required)
license (optional)
compatibility (optional)
metadata (optional, string-to-string map)
Unknown frontmatter fields are ignored.

Validate names
name must:

Be 1–64 characters
Be lowercase alphanumeric with single hyphen separators
Not start or end with -
Not contain consecutive --
Match the directory name that contains SKILL.md
Equivalent regex:

^[a-z0-9]+(-[a-z0-9]+)*$

Follow length rules
description must be 1-1024 characters. Keep it specific enough for the agent to choose correctly.

Use an example
Create .opencode/skills/git-release/SKILL.md like this:

---
name: git-release
description: Create consistent releases and changelogs
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: github
---

## What I do

- Draft release notes from merged PRs
- Propose a version bump
- Provide a copy-pasteable `gh release create` command

## When to use me

Use this when you are preparing a tagged release.
Ask clarifying questions if the target versioning scheme is unclear.

Recognize tool description
OpenCode lists available skills in the skill tool description. Each entry includes the skill name and description:

<available_skills>
  <skill>
    <name>git-release</name>
    <description>Create consistent releases and changelogs</description>
  </skill>
</available_skills>

The agent loads a skill by calling the tool:

skill({ name: "git-release" })

Configure permissions
Control which skills agents can access using pattern-based permissions in opencode.json:

{
  "permission": {
    "skill": {
      "*": "allow",
      "pr-review": "allow",
      "internal-*": "deny",
      "experimental-*": "ask"
    }
  }
}

Permission	Behavior
allow	Skill loads immediately
deny	Skill hidden from agent, access rejected
ask	User prompted for approval before loading
Patterns support wildcards: internal-* matches internal-docs, internal-tools, etc.

Override per agent
Give specific agents different permissions than the global defaults.

For custom agents (in agent frontmatter):

---
permission:
  skill:
    "documents-*": "allow"
---

For built-in agents (in opencode.json):

{
  "agent": {
    "plan": {
      "permission": {
        "skill": {
          "internal-*": "allow"
        }
      }
    }
  }
}

Disable the skill tool
Completely disable skills for agents that shouldn’t use them:

For custom agents:

---
tools:
  skill: false
---

For built-in agents:

{
  "agent": {
    "plan": {
      "tools": {
        "skill": false
      }
    }
  }
}

When disabled, the <available_skills> section is omitted entirely.

Troubleshoot loading
If a skill does not show up:

Verify SKILL.md is spelled in all caps
Check that frontmatter includes name and description
Ensure skill names are unique across all locations
Check permissions—skills with deny are hidden from agents

https://opencode.ai/docs/mcp-servers/

MCP servers
Add local and remote MCP tools.

You can add external tools to OpenCode using the Model Context Protocol, or MCP. OpenCode supports both local and remote servers.

Once added, MCP tools are automatically available to the LLM alongside built-in tools.

Caveats
When you use an MCP server, it adds to the context. This can quickly add up if you have a lot of tools. So we recommend being careful with which MCP servers you use.

Tip

MCP servers add to your context, so you want to be careful with which ones you enable.

Certain MCP servers, like the GitHub MCP server, tend to add a lot of tokens and can easily exceed the context limit.

Enable
You can define MCP servers in your OpenCode Config under mcp. Add each MCP with a unique name. You can refer to that MCP by name when prompting the LLM.

opencode.jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "name-of-mcp-server": {
      // ...
      "enabled": true,
    },
    "name-of-other-mcp-server": {
      // ...
    },
  },
}

You can also disable a server by setting enabled to false. This is useful if you want to temporarily disable a server without removing it from your config.

Overriding remote defaults
Organizations can provide default MCP servers via their .well-known/opencode endpoint. These servers may be disabled by default, allowing users to opt-in to the ones they need.

To enable a specific server from your organization’s remote config, add it to your local config with enabled: true:

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "jira": {
      "type": "remote",
      "url": "https://jira.example.com/mcp",
      "enabled": true
    }
  }
}

Your local config values override the remote defaults. See config precedence for more details.

Local
Add local MCP servers using type to "local" within the MCP object.

opencode.jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "my-local-mcp-server": {
      "type": "local",
      // Or ["bun", "x", "my-mcp-command"]
      "command": ["npx", "-y", "my-mcp-command"],
      "enabled": true,
      "environment": {
        "MY_ENV_VAR": "my_env_var_value",
      },
    },
  },
}

The command is how the local MCP server is started. You can also pass in a list of environment variables as well.

For example, here’s how you can add the test @modelcontextprotocol/server-everything MCP server.

opencode.jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "mcp_everything": {
      "type": "local",
      "command": ["npx", "-y", "@modelcontextprotocol/server-everything"],
    },
  },
}

And to use it I can add use the mcp_everything tool to my prompts.

use the mcp_everything tool to add the number 3 and 4

Options
Here are all the options for configuring a local MCP server.

Option	Type	Required	Description
type	String	Y	Type of MCP server connection, must be "local".
command	Array	Y	Command and arguments to run the MCP server.
cwd	String		Working directory for the MCP server process. Relative paths resolve from the workspace.
environment	Object		Environment variables to set when running the server.
enabled	Boolean		Enable or disable the MCP server on startup.
timeout	Number		Timeout in ms for fetching tools from the MCP server. Defaults to 5000 (5 seconds).
Remote
Add remote MCP servers by setting type to "remote".

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "my-remote-mcp": {
      "type": "remote",
      "url": "https://my-mcp-server.com",
      "enabled": true,
      "headers": {
        "Authorization": "Bearer MY_API_KEY"
      }
    }
  }
}

The url is the URL of the remote MCP server and with the headers option you can pass in a list of headers.

Options
Option	Type	Required	Description
type	String	Y	Type of MCP server connection, must be "remote".
url	String	Y	URL of the remote MCP server.
enabled	Boolean		Enable or disable the MCP server on startup.
headers	Object		Headers to send with the request.
oauth	Object		OAuth authentication configuration. See OAuth section below.
timeout	Number		Timeout in ms for fetching tools from the MCP server. Defaults to 5000 (5 seconds).
OAuth
OpenCode automatically handles OAuth authentication for remote MCP servers. When a server requires authentication, OpenCode will:

Detect the 401 response and initiate the OAuth flow
Use Dynamic Client Registration (RFC 7591) if supported by the server
Store tokens securely for future requests
Automatic
For most OAuth-enabled MCP servers, no special configuration is needed. Just configure the remote server:

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "my-oauth-server": {
      "type": "remote",
      "url": "https://mcp.example.com/mcp"
    }
  }
}

If the server requires authentication, OpenCode will prompt you to authenticate when you first try to use it. If not, you can manually trigger the flow with opencode mcp auth <server-name>.

Pre-registered
If you have client credentials from the MCP server provider, you can configure them:

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "my-oauth-server": {
      "type": "remote",
      "url": "https://mcp.example.com/mcp",
      "oauth": {
        "clientId": "{env:MY_MCP_CLIENT_ID}",
        "clientSecret": "{env:MY_MCP_CLIENT_SECRET}",
        "scope": "tools:read tools:execute"
      }
    }
  }
}

Authenticating
You can manually trigger authentication or manage credentials.

Authenticate with a specific MCP server:

Terminal window
opencode mcp auth my-oauth-server

List all MCP servers and their auth status:

Terminal window
opencode mcp list

Remove stored credentials:

Terminal window
opencode mcp logout my-oauth-server

The mcp auth command will open your browser for authorization. After you authorize, OpenCode will store the tokens securely in ~/.local/share/opencode/mcp-auth.json.

Disabling OAuth
If you want to disable automatic OAuth for a server (e.g., for servers that use API keys instead), set oauth to false:

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "my-api-key-server": {
      "type": "remote",
      "url": "https://mcp.example.com/mcp",
      "oauth": false,
      "headers": {
        "Authorization": "Bearer {env:MY_API_KEY}"
      }
    }
  }
}

OAuth Options
Option	Type	Description
oauth	Object | false	OAuth config object, or false to disable OAuth auto-detection.
clientId	String	OAuth client ID. If not provided, dynamic client registration will be attempted.
clientSecret	String	OAuth client secret, if required by the authorization server.
scope	String	OAuth scopes to request during authorization.
Debugging
If a remote MCP server is failing to authenticate, you can diagnose issues with:

Terminal window
# View auth status for all OAuth-capable servers
opencode mcp auth list

# Debug connection and OAuth flow for a specific server
opencode mcp debug my-oauth-server

The mcp debug command shows the current auth status, tests HTTP connectivity, and attempts the OAuth discovery flow.

Manage
Your MCPs are available as tools in OpenCode, alongside built-in tools. So you can manage them through the OpenCode config like any other tool.

Global
This means that you can enable or disable them globally.

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "my-mcp-foo": {
      "type": "local",
      "command": ["bun", "x", "my-mcp-command-foo"]
    },
    "my-mcp-bar": {
      "type": "local",
      "command": ["bun", "x", "my-mcp-command-bar"]
    }
  },
  "tools": {
    "my-mcp-foo": false
  }
}

We can also use a glob pattern to disable all matching MCPs.

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "my-mcp-foo": {
      "type": "local",
      "command": ["bun", "x", "my-mcp-command-foo"]
    },
    "my-mcp-bar": {
      "type": "local",
      "command": ["bun", "x", "my-mcp-command-bar"]
    }
  },
  "tools": {
    "my-mcp*": false
  }
}

Here we are using the glob pattern my-mcp* to disable all MCPs.

Per agent
If you have a large number of MCP servers you may want to only enable them per agent and disable them globally. To do this:

Disable it as a tool globally.
In your agent config, enable the MCP server as a tool.
opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "my-mcp": {
      "type": "local",
      "command": ["bun", "x", "my-mcp-command"],
      "enabled": true
    }
  },
  "tools": {
    "my-mcp*": false
  },
  "agent": {
    "my-agent": {
      "tools": {
        "my-mcp*": true
      }
    }
  }
}

Glob patterns
The glob pattern uses simple regex globbing patterns:

* matches zero or more of any character (e.g., "my-mcp*" matches my-mcp_search, my-mcp_list, etc.)
? matches exactly one character
All other characters match literally
Note

MCP server tools are registered with server name as prefix, so to disable all tools for a server simply use:

"mymcpservername_*": false

Examples
Below are examples of some common MCP servers. You can submit a PR if you want to document other servers.

Sentry
Add the Sentry MCP server to interact with your Sentry projects and issues.

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "sentry": {
      "type": "remote",
      "url": "https://mcp.sentry.dev/mcp",
      "oauth": {}
    }
  }
}

After adding the configuration, authenticate with Sentry:

Terminal window
opencode mcp auth sentry

This will open a browser window to complete the OAuth flow and connect OpenCode to your Sentry account.

Once authenticated, you can use Sentry tools in your prompts to query issues, projects, and error data.

Show me the latest unresolved issues in my project. use sentry

Context7
Add the Context7 MCP server to search through docs.

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "context7": {
      "type": "remote",
      "url": "https://mcp.context7.com/mcp"
    }
  }
}

If you have signed up for a free account, you can use your API key and get higher rate-limits.

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "context7": {
      "type": "remote",
      "url": "https://mcp.context7.com/mcp",
      "headers": {
        "CONTEXT7_API_KEY": "{env:CONTEXT7_API_KEY}"
      }
    }
  }
}

Here we are assuming that you have the CONTEXT7_API_KEY environment variable set.

Add use context7 to your prompts to use Context7 MCP server.

Configure a Cloudflare Worker script to cache JSON API responses for five minutes. use context7

Alternatively, you can add something like this to your AGENTS.md.

AGENTS.md
When you need to search docs, use `context7` tools.

Grep by Vercel
Add the Grep by Vercel MCP server to search through code snippets on GitHub.

opencode.json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "gh_grep": {
      "type": "remote",
      "url": "https://mcp.grep.app"
    }
  }
}

Since we named our MCP server gh_grep, you can add use the gh_grep tool to your prompts to get the agent to use it.

What's the right way to set a custom domain in an SST Astro component? use the gh_grep tool

Alternatively, you can add something like this to your AGENTS.md.
