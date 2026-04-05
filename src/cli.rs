use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "folio")]
#[command(about = "Markdown file management CLI for AI agents")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output in human-readable format instead of JSON
    #[arg(long, global = true)]
    pub pretty: bool,

    /// Specify workspace path
    #[arg(long, global = true)]
    pub workspace: Option<String>,

    /// Limit scanning to a subdirectory (relative to workspace root)
    #[arg(long, global = true)]
    pub scope: Option<String>,

    /// Skip indexing, operate directly on files
    #[arg(long, global = true)]
    pub no_index: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new markdown file
    New {
        /// Path for the new file
        path: String,
        /// Template to apply
        #[arg(long)]
        template: Option<String>,
        /// Initial content (from stdin if not provided)
        #[arg(long)]
        content: Option<String>,
    },

    /// Output file content as JSON
    Cat {
        /// Path to the file
        path: String,
    },

    /// Edit file operations
    Edit {
        #[command(subcommand)]
        action: EditAction,
    },

    /// Delete a markdown file
    Rm {
        /// Path to the file
        path: String,
    },

    /// Move/rename a file (updates link references)
    Mv {
        /// Source path
        src: String,
        /// Destination path
        dst: String,
    },

    /// List markdown files
    Ls {
        /// Directory path to list
        path: Option<String>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Filter by frontmatter field (key=value)
        #[arg(long)]
        filter: Option<String>,
    },

    /// Run a query expression
    Query {
        /// Query expression
        expression: String,
    },

    /// Full-text search
    Search {
        /// Search text
        text: String,
    },

    /// List all tags with counts
    Tags,

    /// Show link relationships
    Graph {
        /// Path to a specific file
        path: Option<String>,
        /// Show full workspace graph
        #[arg(long)]
        full: bool,
        /// Find files with no links
        #[arg(long)]
        orphans: bool,
        /// Find shortest link path between two files
        #[arg(long, num_args = 2)]
        path_between: Option<Vec<String>>,
    },

    /// Template operations
    Template {
        #[command(subcommand)]
        action: TemplateAction,
    },

    /// Batch operations
    Batch {
        #[command(subcommand)]
        action: BatchAction,
    },

    /// Initialize a workspace
    Init,

    /// Show workspace status
    Status,

    /// Rebuild index
    Index,

    /// Plugin operations
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Subcommand)]
pub enum EditAction {
    /// Modify a frontmatter field
    Frontmatter {
        /// File path
        path: String,
        /// Field key
        key: String,
        /// Field value
        value: String,
    },
    /// Replace a section by heading
    Section {
        /// File path
        path: String,
        /// Heading to find
        heading: String,
        /// New content
        #[arg(long)]
        content: Option<String>,
    },
    /// Append content to a file
    Append {
        /// File path
        path: String,
        /// Content to append
        #[arg(long)]
        content: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum TemplateAction {
    /// List available templates
    List,
    /// Apply a template to create a file
    Apply {
        /// Template name
        name: String,
        /// Output file path
        path: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum BatchAction {
    /// Batch set frontmatter fields
    Set {
        /// Key=value pairs to set
        pairs: Vec<String>,
        /// Query to select files
        #[arg(long)]
        query: Option<String>,
        /// Glob pattern to select files
        #[arg(long)]
        glob: Option<String>,
        /// Preview changes without executing
        #[arg(long)]
        dry_run: bool,
    },
    /// Batch tag operations
    Tag {
        /// "add" or "remove"
        action: String,
        /// Tag name
        tag: String,
        /// Query to select files
        #[arg(long)]
        query: Option<String>,
        /// Glob pattern to select files
        #[arg(long)]
        glob: Option<String>,
        /// Preview changes without executing
        #[arg(long)]
        dry_run: bool,
    },
    /// Batch move files
    Move {
        /// Destination directory
        #[arg(long)]
        dest: String,
        /// Query to select files
        #[arg(long)]
        query: Option<String>,
        /// Preview changes without executing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum PluginAction {
    /// List loaded plugins
    List,
    /// Run a plugin command
    Run {
        /// Plugin name
        name: String,
        /// Plugin subcommand and arguments (use -- to separate plugin args from folio args)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
