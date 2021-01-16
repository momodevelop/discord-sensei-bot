use rusqlite::Connection;
use rusqlite::NO_PARAMS;

use serenity::async_trait;
use serenity::client::Client;
use serenity::client::Context;
use serenity::client::EventHandler;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::macros::group;
use serenity::framework::standard::StandardFramework;
use serenity::framework::standard::CommandResult;
use serenity::model::channel::Message;
use serenity::model::id::UserId;
use serenity::model::gateway::Activity;
use serenity::model::gateway::Ready;
use serenity::prelude::TypeMapKey;

use serde::Deserialize;

use std::fs::File;
use std::io::BufReader;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use tokio::sync::Mutex;



// TypeMapKeys ////////////////////////////////////////////////////////////////////
struct Database;
impl TypeMapKey for Database {
    type Value = Mutex<Connection>;
}


// Common functions ///////////////////////////////////////////////////////////////
macro_rules! help_queue {
    () => ( "queue: Use this command to queue for consultation\n\t> Usage: !consult queue\n" )
}

macro_rules! help_unqueue {
    () =>( "unqueue: Use this command to unqueue for consultation\n\t>Usage: !consult unqueue\n" )
}

macro_rules! help {
    () => (concat!(help_queue!(), help_unqueue!()));
}

macro_rules! wrap_code {
    ($item:expr) => (concat!("```", $item, "```"))
}

async fn say(ctx: &Context, msg: &Message, display: impl std::fmt::Display)  {
    if let Err(why) = msg.channel_id.say(&ctx.http, display).await {
        println!("Error sending message: {:?}", why);
    }
}


fn is_user_queued(discord_id: UserId, db: &Connection) -> bool {
    let mut count: u32 = 0;
    {
        let query = "SELECT COUNT(discord_id) FROM queue WHERE discord_id = (?)";
        let mut stmt = db.prepare(query).unwrap();
        let mut rows = stmt.query(&[&discord_id.to_string()]).unwrap();
        if let Some(row) = rows.next().unwrap() {
            count = row.get(0).unwrap();
        }
    }

    return count > 0;
}


// Commands //////////////////////////////////////////////////////////////////////////
#[group]
#[commands(version, help, queue, unqueue, list, remove, when)]
struct General;

// Commands allowed by students
#[command]
async fn version(ctx: &Context, msg: &Message) -> CommandResult {
    say(ctx, msg, "I'm SenseiBot v1.0.0, written in Rust!!").await;
    return Ok(());
}

#[command]
async fn help(ctx: &Context, msg: &Message) -> CommandResult {
    say(ctx, msg, wrap_code!(help!())).await;
    return Ok(());
}

#[command]
async fn queue(ctx: &Context, msg: &Message) -> CommandResult {
    // Check if discord id exists
    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap().lock().await;
   
    // Check in an entry already exists
    {
        let mut count: u32 = 0;
        {
            let query = "SELECT COUNT(discord_id) FROM queue WHERE discord_id = (?)";
            let mut stmt = db.prepare(query).unwrap();
            let mut rows = stmt.query(&[&msg.author.id.to_string()]).unwrap();
            if let Some(row) = rows.next().unwrap() {
                count = row.get(0).unwrap();
            }
        }
        
        if count > 0 {
            say(ctx, msg, "You have already queued!").await;
            return Ok(());
        }

    }

    // Insert into the database
    {
        let since_the_epoch = SystemTime::now().duration_since(UNIX_EPOCH)
                                               .expect("Time went backwards");
        let query = "INSERT INTO queue (`discord_id`, `name`, `created`) VALUES (?,?,?)";
        let rows_affected = db.execute(query, &[
                                          &msg.author.id.to_string(),
                                          &msg.author.name,
                                          &since_the_epoch.as_millis().to_string(), 
                                       ]).unwrap();
        if rows_affected == 0 {
            say(ctx, msg, "Sorry, there was a problem queuing you in. Try DMing sensei.").await;
            return Ok(());
        }
    }

    //Get the queue number
    {
        let mut queue_length: u32 = 0;
        {
            let query = "SELECT COUNT(*) FROM queue";
            let mut stmt = db.prepare(query).unwrap();
            let mut rows = stmt.query(NO_PARAMS).unwrap();
            if let Some(row) = rows.next().unwrap() {
                queue_length = row.get(0).unwrap();    
            }
        }
        say(ctx, msg, format!("Queued! You are **{}** on the queue", queue_length)).await;
    }

    return Ok(());
}


#[command]
async fn unqueue(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap().lock().await;

    let query = "DELETE FROM queue WHERE discord_id = (?)";
    let rows_affected = db.execute(query, &[&msg.author.id.to_string()]).unwrap();

    if rows_affected > 0 {
        say(ctx, msg, "You have been successfully removed from the queue").await;
    } else {
        say(ctx, msg, "You were not in the queue!").await;
    }
    return Ok(());
}

#[command]
async fn when(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap().lock().await;

    if !is_user_queued(msg.author.id, &db) {
        say(ctx, msg, "You are not in the queue!").await;
        return Ok(());
    }

    let mut queue_number_opt: Option<u32> = None; 
    {
        let query = "SELECT COUNT(*) FROM queue WHERE created < (
                        SELECT created FROM queue WHERE discord_id = (?)
                     )";

        let mut stmt = db.prepare(query).unwrap();
        let mut rows = stmt.query(&[&msg.author.id.to_string()]).unwrap();
        if let Some(row) = rows.next().unwrap() {
            queue_number_opt = Some(row.get(0).unwrap());  
        }
    }

    if let Some(queue_number) = queue_number_opt {    
        say(ctx, msg, format!("Your queue number is: **{}**", queue_number + 1)).await;
    } else {
        say(ctx, msg, "Something went wrong. Contact sensei!").await;
    }


    return Ok(());
}


// Commands only allowed by me
#[command]
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    say(ctx, msg, "not implemented yet").await;
    return Ok(());
}

#[command]
async fn remove(ctx: &Context, msg: &Message) -> CommandResult {
    say(ctx, msg, "not implemented yet").await;
    return Ok(());
}


#[derive(Deserialize)]
struct Config {
    token: String,
    prefix: String,
    db_path: String,
}

struct Handler; 
#[async_trait] impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        ctx.set_activity(Activity::playing("type !sensei help")).await;
    }
}



#[tokio::main]
async fn main() {
    let mut client: Client;
    {
        let config: Config;
        {
            let file = File::open("config.json").unwrap();
            let reader = BufReader::new(file);
            config = serde_json::from_reader(reader).unwrap();
        }

        let framework = StandardFramework::new()
            .configure(|c| c
                  .with_whitespace(true)
                  .prefix(config.prefix.as_str()))
            .group(&GENERAL_GROUP);
    
        
        client = Client::builder(&config.token)
            .event_handler(Handler)
            .framework(framework)
            .await
            .unwrap();

        let mut data = client.data.write().await;
        // database
        {
            let conn = Connection::open(config.db_path).unwrap();
            data.insert::<Database>(Mutex::new(conn));
            data.insert::<Config>(Mutex::new(config));
        }
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
