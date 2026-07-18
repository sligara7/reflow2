https://code.visualstudio.com/docs/agent-customization/agent-skills

Use Agent Skills in VS Code
Agent Skills are folders of instructions, scripts, and resources that GitHub Copilot can load when relevant to perform specialized tasks. Agent Skills is an open standard that works across multiple AI agents, including GitHub Copilot in VS Code, GitHub Copilot CLI, and GitHub Copilot cloud agent.

Unlike custom instructions that primarily define coding guidelines, skills enable specialized capabilities and workflows that can include scripts, examples, and other resources. Skills you create are portable and work across any skills-compatible agent.

For how skills compare with the other customization options, see Customization concepts.

Key benefits of Agent Skills:

Specialize Copilot: Tailor capabilities for domain-specific tasks without repeating context
Reduce repetition: Create once, use automatically across all conversations
Compose capabilities: Combine multiple skills to build complex workflows
Efficient loading: Only relevant content loads into context when needed
Tip
Use the Agent Customizations editor (Preview) to discover, create, and manage all your agent customizations in one place. Run Chat: Open Customizations from the Command Palette.

Agent Skills vs custom instructions
While both Agent Skills and custom instructions help customize Copilot's behavior, they serve different purposes:

Expand table
Feature	Agent Skills	Custom Instructions
Purpose	Teach specialized capabilities and workflows	Define coding standards and guidelines
Portability	Works across VS Code, Copilot CLI, and Copilot cloud agent	VS Code and GitHub.com only
Content	Instructions, scripts, examples, and resources	Instructions only
Scope	Task-specific, loaded on-demand	Always applied (or via glob patterns)
Standard	Open standard (agentskills.io)	VS Code-specific
Use Agent Skills when you want to:

Create reusable capabilities that work across different AI tools
Include scripts, examples, or other resources alongside instructions
Share capabilities with the wider AI community
Define specialized workflows like testing, debugging, or deployment processes
Use custom instructions when you want to:

Define project-specific coding standards
Set language or framework conventions
Specify code review or commit message guidelines
Apply rules based on file types using glob patterns
Create a skill
Tip
Type /skills in the chat input to quickly open the Configure Skills menu.

Skills are stored in directories with a SKILL.md file that defines the skill's behavior. VS Code supports two types of skills:

Expand table
Skill type	Location
Project skills, stored in your repository	.github/skills/, .claude/skills/, .agents/skills/
Personal skills, stored in your user profile	~/.copilot/skills/, ~/.claude/skills/, ~/.agents/skills/
You can configure additional file locations for project skills with the 
chat.agentSkillsLocations
 setting. This is useful if you want to organize skills in a different folder structure or have multiple skill directories.
Tip
In a monorepo, enable 
chat.useCustomizationsInParentRepositories
 to discover skills from the parent repository root. Learn more about parent repository discovery.
To create a skill:

In the Chat view, select Configure Chat (gear icon) to open the Agent Customizations editor and then select the Skills tab.

Select New Skill (Workspace) or New Skill (User) from the dropdown, depending on where you want to store the skill.

Screenshot of the Agent Customizations editor, showing the Skills tab and the dropdown to create a new skill.

Select the location and enter a name for the skill.

Complete the SKILL.md file by filling in the YAML frontmatter and adding instructions in the body of the file.

Markdown

---
name: skill-name
description: Description of what the skill does and when to use it
---

# Skill Instructions

Your detailed instructions, guidelines, and examples go here...
Optionally, add scripts, examples, or other resources to your skill's directory.

For example, a skill for testing web applications might include:

SKILL.md - Instructions for running tests
test-template.js - A template test file
examples/ - Example test scenarios
Note
Make sure to reference any additional files in your SKILL.md for them to be picked up by the agent. Use Markdown link syntax with relative paths, such as [test template](./test-template.js).

Generate a skill with AI
You can use AI to generate a skill based on a description of the capability. Type /create-skill in chat and describe the skill you want (for example, "a skill for running and debugging integration tests"). The agent asks clarifying questions and generates a SKILL.md file with the directory structure, instructions, and frontmatter.

You can also extract a reusable skill from an ongoing conversation. For example, after a multi-turn session where you debugged a complex issue, ask "create a skill from how we just debugged that" to capture the multi-step procedure as a reusable skill.

You can also generate a skill from the Agent Customizations editor by selecting Generate Skill from the dropdown.

SKILL.md file format
The SKILL.md file is a Markdown file with YAML frontmatter that defines the skill's metadata and behavior.

Header (required)
The header is formatted as YAML frontmatter with the following fields:

Expand table
Field	Required	Description
name	Yes	A unique identifier for the skill. Only lowercase letters, numbers, and hyphens are allowed (for example, webapp-testing). Do not use slashes, colons, dots, or namespace prefixes. Must match the parent directory name. Maximum 64 characters. Names with invalid characters cause the skill to silently fail to load.
description	Yes	A description of what the skill does and when to use it. Be specific about both capabilities and use cases to help Copilot decide when to load the skill. Maximum 1024 characters.
argument-hint	No	Hint text shown in the chat input field when the skill is invoked as a slash command. Helps users understand what additional information to provide (for example, [test file] [options]).
user-invocable	No	Controls whether the skill appears as a slash command in the chat menu. Defaults to true. Set to false to hide the skill from the / menu while still allowing the agent to load it automatically.
disable-model-invocation	No	Controls whether the agent can automatically load the skill based on relevance. Defaults to false. Set to true to require manual invocation through the / slash command only.
context	No	(Experimental) Controls how the skill is loaded. Defaults to inline (the skill's instructions are added to the parent agent's context). Set to fork to run the skill in a dedicated subagent context. See Run a skill in a forked context.
Important
When a skill is distributed through a plugin, the plugin name is automatically used as a command prefix (for example, /my-plugin:test-runner). Do not manually add namespace prefixes to the skill name field. Using prefixes like myorg/skillname or myorg:skillname causes the skill to silently fail to load.

Body
The skill body contains the instructions, guidelines, and examples that Copilot should follow when using this skill. Write clear, specific instructions that describe:

What the skill helps accomplish
When to use the skill
Step-by-step procedures to follow
Examples of the expected input and output
References to any included scripts or resources
You can reference files within the skill directory using relative paths. For example, to reference a script in your skill directory, use [test script](./test-template.js).

Run a skill in a forked context (experimental)
By default, when VS Code loads a skill, the skill's instructions are added to the parent agent's context window. For large skills, or skills whose intermediate reasoning isn't relevant to the rest of your conversation, you can instead run the skill in a forked context. In a forked context, the skill executes in a dedicated subagent and only its final result is returned to the parent agent. This keeps your main conversation's context clean.

To run a skill in a forked context, set the context field in the SKILL.md frontmatter to fork:

Markdown

---
name: review-pr
description: Review a pull request for code quality, style, and correctness. Use when asked to review a PR.
context: fork
---

# PR review

Follow these steps to review the pull request...
Use context: fork for skills that:

Read many files or run lengthy investigations whose details don't need to stay in the main conversation
Produce a focused result (such as a summary, a report, or a small set of edits) that the parent agent can act on directly
Should not influence the parent agent's behavior beyond their final output
Note
Running a skill in a forked context is an experimental feature. Enable the 
github.copilot.chat.skillTool.enabled
 setting in VS Code to use this feature.
Example skills
The following examples demonstrate different types of skills you can create.

Example: Web application testing skill
Example: GitHub Actions debugging skill
Use skills as slash commands
Skills are available as slash commands in chat, alongside prompt files. Type / in the chat input field to see a list of available skills and prompts, and select a skill to invoke it.

You can add extra context after the slash command. For example, /webapp-testing for the login page or /github-actions-debugging PR #42.

By default, all skills appear in the / menu. Use the user-invocable and disable-model-invocation frontmatter properties to control how each skill is accessed:

Expand table
Configuration	Slash command	Auto-loaded by Copilot	Use case
Default (both properties omitted)	Yes	Yes	General-purpose skills
user-invocable: false	No	Yes	Background knowledge skills that the model loads when relevant
disable-model-invocation: true	Yes	No	Skills you only want to run on demand
Both set	No	No	Disabled skills
How Copilot uses skills
Skills load content progressively to keep your context efficient. Here is an example of how Copilot uses the webapp-testing skill:

Discovery: Copilot reads the skill's name and description from the YAML frontmatter. When you ask "help me test the login page", Copilot matches this to the webapp-testing skill based on its description.

Instructions loading: Copilot loads the SKILL.md body into its context, giving it access to the detailed testing procedures and guidelines. You can also trigger this step directly by typing /webapp-testing in chat.

Resource access: As Copilot works through the instructions, it accesses additional files in the skill directory, such as test-template.js or example scenarios, only when it references them. If a file isn't referenced in the instructions, it won't be loaded.

This three-level loading system means you can install many skills without consuming context. Copilot loads only what is relevant for each task.

Skills that opt in to a forked context follow the same discovery step, but their instructions and any files they read are loaded into a separate subagent. Only the skill's final result is returned to the parent agent.

Use shared skills
You can use skills created by others to enhance Copilot's capabilities. The github/awesome-copilot repository contains a growing community collection of skills, custom agents, instructions, and prompts. The anthropics/skills repository contains additional reference skills.

You can also discover and install skills that are bundled in agent plugins. Skills from installed plugins appear alongside your locally defined skills in the Configure Skills menu.

To use a shared skill:

Browse the available skills in the repository
Copy the skill directory to your .github/skills/ folder
Review and customize the SKILL.md file for your needs
Optionally, modify or add resources as needed
Tip
Always review shared skills before using them to ensure they meet your requirements and security standards. VS Code's terminal tool provides controls for script execution, including auto-approve options with configurable allow-lists and tight controls over which code runs. Learn more about security considerations for auto-approval features.

Contribute skills from extensions
Extensions can contribute skills using the chatSkills contribution point in their package.json. The path must point to a directory that contains a SKILL.md file, following the Agent Skills specification.

Required folder structure
The skill directory must follow this structure:

Text

extension-root/
└── skills/
    └── my-skill/           # Directory name must match the `name` field in SKILL.md
        └── SKILL.md         # Required
Register the skill in package.json
Add the chatSkills contribution point in your extension's package.json. The path property must point to the corresponding SKILL.md file:

JSON

{
  "contributes": {
    "chatSkills": [
      {
        "path": "./skills/my-skill/SKILL.md"
      }
    ]
  }
}
Important
The name field in the SKILL.md frontmatter must match the parent directory name. For example, if the directory is skills/my-skill/, the name field must be my-skill. If the name does not match, the skill is not loaded.

The SKILL.md file follows the same format as project and personal skills. For example:

Markdown

---
name: my-skill
description: Description of what the skill does and when to use it.
---

# My Skill

Detailed instructions for the skill...
Agent Skills standard
Agent Skills is an open standard that enables portability across different AI agents. Skills you create in VS Code work with multiple agents, including:

GitHub Copilot in VS Code: Available in chat and agent mode
GitHub Copilot CLI: Accessible when working in the terminal
GitHub Copilot cloud agent: Used during automated coding tasks
Learn more about the Agent Skills standard at agentskills.io.

Related resources
Customize AI responses overview
Create custom instructions
Create reusable prompt files
Create custom agents
Agent Skills specification
Reference skills repository
Discover and manage agent plugins


https://code.visualstudio.com/docs/agent-customization/mcp-servers
Add and manage MCP servers in VS Code
Model Context Protocol (MCP) is an open standard for connecting AI models to external tools and services. In Visual Studio Code, MCP servers provide tools for tasks like file operations, databases, or external APIs. MCP servers can also provide resources, prompts, and interactive apps.

For background on how MCP fits into the AI customization framework, see Customization concepts and Tools concepts.

This article covers how to add, configure, and manage MCP servers. To learn about using tools in chat, see Use tools in chat.

Tip
Use the Agent Customizations editor (Preview) to discover, create, and manage all your agent customizations in one place. Run Chat: Open Customizations from the Command Palette.

Quickstart: use an MCP server in chat
Follow these steps to install an MCP server and use its tools in chat. This example uses the Playwright MCP server to interact with web pages through a browser.

Open the Extensions view (Ctrl+Shift+X) and enter @mcp playwright in the search field.

Select Install to install the Playwright MCP server in your user profile.

When prompted, confirm that you trust the server to start it. VS Code discovers the server's tools and makes them available in chat.

Open the Chat view (Ctrl+cmd+I) and enter a prompt that uses the Playwright tools. For example:

Prompt
Open in VS Code


Go to code.visualstudio.com, decline the cookie banner, and give me a screenshot of the homepage.
VS Code invokes the Playwright tools to open the page in a browser, and take a screenshot. You might be asked to confirm each tool invocation.

Tip
Select the Configure Tools button in the chat input to see all available tools for the Playwright MCP server and toggle specific tools on or off.

Add an MCP server
To install an MCP server from the MCP server gallery:

Open the Extensions view (Ctrl+Shift+X) and enter @mcp in the search field. This shows the list of available MCP servers in the gallery.

You can install an MCP server in your user profile or in your workspace:

To install in your user profile, select Install.

To install in your workspace, right-click the MCP server and select Install in Workspace. This updates the .vscode/mcp.json file in your workspace.

To view the MCP server details, select the MCP server in the list to open the details page.

Caution
Local MCP servers can run arbitrary code on your machine. Only add servers from trusted sources, and review the publisher and server configuration before starting it. Read the Security documentation for using AI in VS Code to understand the implications.

Configure the mcp.json file
You can manually configure MCP servers by editing the mcp.json file. There are two locations for this file:

Workspace: create or open .vscode/mcp.json in your project. Include this file in source control to share MCP server configurations with your team.
User profile: run the MCP: Open User Configuration command to open the mcp.json file in your user profile folder. Servers configured here are available across all your workspaces. When you use multiple profiles, each profile can have its own MCP server configuration.
You can also run MCP: Add Server in the Command Palette (Shift+cmd+P) to add a server through a guided flow, choosing either Workspace or Global as the target.

Important
Avoid hardcoding sensitive information like API keys. Use input variables or environment files instead.

The following example shows an mcp.json file that configures a remote MCP server and a local MCP server:

JSON

{
  "servers": {
    "github": {
      "type": "http",
      "url": "https://api.githubcopilot.com/mcp"
    },
    "playwright": {
      "command": "npx",
      "args": ["-y", "@microsoft/mcp-server-playwright"]
    }
  }
}
VS Code provides IntelliSense for the configuration file. For the full configuration schema and field reference, see the MCP configuration reference.

Note
MCP servers run wherever they are configured. Servers in your user profile run locally. If you're connected to a remote and want a server to run on the remote machine, define it in the workspace settings or remote user settings (MCP: Open Remote User Configuration).

Other options to add an MCP server
Add an MCP server to a dev container
Automatically discover MCP servers
Install an MCP server from the command line
Other MCP capabilities
Beyond tools, MCP servers can provide other capabilities:

Expand table
Capability	Description	How to use
Resources	Access data from MCP servers as context in your prompts, such as files, database tables, or API responses. Resources provide read-only context that you attach to a chat request.	In the Chat view, select Add Context > MCP Resources. You can also use the MCP: Browse Resources command.
Prompts	Use preconfigured prompt templates from MCP servers to standardize common tasks. Each MCP server can expose its own set of prompts tailored to its capabilities.	Type /<MCP server>.<prompt> in the chat input.
MCP Apps	Get interactive UI components like forms, visualizations, and drag-and-drop lists rendered directly in chat. MCP Apps enable richer interactions beyond text responses. Learn more in the MCP Apps blog post.	MCP Apps appear inline when an MCP server supports them.
Sandbox MCP servers
On macOS and Linux, you can enable sandboxing for locally-running stdio MCP servers to restrict their access to the file system and network. Sandboxed servers run in an isolated environment and can only access the file paths and network domains that you explicitly permit.

To enable sandboxing for a server, set "sandboxEnabled": true in the server configuration in your mcp.json file. You can further customize the sandbox restrictions by adding a top-level sandbox object with specific file system and network rules.

The following example shows how to enable sandboxing for a local MCP server and restrict its access to only write to files in the workspace and access a specific API domain:

JSON

{
  "servers": {
    "myServer": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@example/mcp-server"],
      "sandboxEnabled": true
    }
  },
  "sandbox": {
    "filesystem": {
      "allowWrite": ["${workspaceFolder}"]
    },
    "network": {
      "allowedDomains": ["api.example.com"]
    }
  }
}
When sandboxing is enabled, tool calls from the server are auto-approved because they run in a controlled environment.

For the full sandbox configuration schema, see the Sandbox configuration reference.

Note
Sandboxing is currently not available on Windows.

Manage MCP servers
VS Code provides several options to manage your MCP servers, such as starting or stopping a server, viewing logs, uninstalling, or clearing cached tools.

Expand table
Method	Description	
Extensions view	Right-click a server in the MCP SERVERS - INSTALLED section or select the gear icon.	Screenshot showing the MCP servers in the Extensions view.
mcp.json editor	Open the configuration file and use the inline actions (code lenses). Use MCP: Open User Configuration or MCP: Open Workspace Folder Configuration to open the file.	MCP server configuration with lenses to manage server.
Command Palette	Run MCP: List Servers, select a server, and choose an action.	Screenshot showing the actions for an MCP server in the Command Palette.
Enable or disable MCP servers
You can enable or disable an MCP server globally or for a specific workspace. When an MCP server is disabled, it does not start and its tools, prompts, resources, and MCP apps are excluded from chat.

To enable or disable an MCP server:

Right-click a server in the MCP SERVERS - INSTALLED section of the Extensions view and select Enable or Disable.
Run MCP: List Servers from the Command Palette, select a server, and choose Enable or Disable.
Use the Agent Customizations editor to toggle the server's enabled state.
The enable/disable state is stored separately from the server configuration in mcp.json, so it does not affect shared configuration files.

Centrally manage access to MCP servers in VS Code
Organizations can centrally manage access to MCP servers via GitHub policies. Learn more about enterprise management of MCP servers.

Automatically start MCP servers
When you add an MCP server or change its configuration, VS Code needs to (re)start the server to discover the tools it provides.

You can configure VS Code to automatically restart the MCP server when configuration changes are detected by using the 
chat.mcp.autoStart
 setting (Experimental).
MCP server trust
When you add an MCP server to your workspace or change its configuration, you need to confirm that you trust the server and its capabilities before starting it. VS Code shows a dialog to confirm that you trust the server when you start a server for the first time. In the dialog, select the link to the MCP server to review its configuration.

Screenshot showing the MCP server trust prompt.

If you don't trust the MCP server, it will not be started, and chat requests will continue without using the tools provided by the server.

You can reset trust for your MCP servers by running the MCP: Reset Trust command from the Command Palette.

Warning
If you start the MCP server directly from the mcp.json file, you will not be prompted to trust the server configuration.

Synchronize MCP configuration across devices
With Settings Sync enabled, you can synchronize settings and configurations across devices, including MCP server configurations. This enables you to maintain a consistent development environment and access the same MCP servers on all your devices.

To synchronize MCP server configuration with Settings Sync:

Run the Settings Sync: Configure command from the Command Palette

Enable the MCP Servers option in the list of synchronized configurations

Troubleshoot and debug MCP servers
MCP output log
When VS Code encounters an issue with an MCP server, it shows an error indicator in the Chat view.

MCP Server Error

Select the error notification in the Chat view, and then select the Show Output option to view the server logs. Alternatively, run MCP: List Servers from the Command Palette, select the server, and then choose Show Output.

MCP Server Error Output

Frequently asked questions
The MCP server is not starting when using Docker
Related resources
MCP configuration reference
Use tools in chat
Model Context Protocol Documentation
MCP Apps support in VS Code
Discover and manage agent plugins, including MCP servers in plugins
