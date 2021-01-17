pub const STMT_QUEUE_UP: &str = "INSERT INTO queue (`discord_id`, `name`, `note`, 
`created`) VALUES (?,?,?,?)";
pub const STMT_QUEUE_COUNT: &str = "SELECT COUNT(*) FROM queue";
pub const STMT_QUEUE_ENTRY_EXIST: &str = "SELECT COUNT(discord_id) FROM queue WHERE discord_id = (?)";
pub const STMT_UNQUEUE: &str = "DELETE FROM queue WHERE discord_id = (?)";
pub const STMT_QUEUE_NUMBER: &str = "SELECT COUNT(*) FROM queue WHERE created <= (SELECT created FROM queue WHERE discord_id = (?))";
pub const STMT_LIST: &str =  "SELECT discord_id, name, note, created FROM queue ORDER BY created DESC";
pub const STMT_REMOVE_ENTRY: &str = "DELETE FROM queue WHERE discord_id = (?)";

pub const MSG_ERROR: &str  =  "Sorry, there was a problem. Try DMing sensei.";
pub const MSG_QUEUE_ALREADY: &str = "You have already queued";
pub const MSG_REMOVE_QUEUE_SUCCESS: &str = "You have been successfully removed form the queue";
pub const MSG_NOT_IN_QUEUE: &str = "Not in the queue!";
pub const MSG_EMPTY_LIST: &str = "No one is looking for consultation";
pub const MSG_MISSING_DISCORD_ID: &str = "Please provide a discord id";
pub const MSG_INVALID_USER_ID: &str = "Id is not valid UserId";
pub const MSG_DISCORD_ID_NOT_EXIST: &str = "discord_id does not exist";

pub const DISCORD_MSG_LIMIT: usize = 2000;
