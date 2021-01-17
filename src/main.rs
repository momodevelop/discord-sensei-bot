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
use serenity::framework::standard::Args;
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

mod constants;
use crate::constants::*;

// TypeMapKeys ////////////////////////////////////////////////////////////////////
struct Database;
impl TypeMapKey for Database {
    type Value = Mutex<Connection>;
}

struct OwnerId;
impl TypeMapKey for OwnerId {
    type Value = UserId;
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
        let mut stmt = db.prepare(STMT_QUEUE_ENTRY_EXIST).unwrap();
        let mut rows = stmt.query(&[&discord_id.to_string()]).unwrap();
        if let Some(row) = rows.next().unwrap() {
            count = row.get(0).unwrap();
        }
    }

    return count > 0;
}

fn args_to_string(mut args: Args) -> String {
    let mut ret = String::with_capacity(128);
    ret.push_str(args.single::<String>().unwrap().as_str());
    for arg in args.iter::<String>() {
        ret.push_str(format!(" {}", arg.unwrap()).as_str());
    }

    return ret;
}

// Commands ///////////////////////////////
#[group]
#[commands(version, help, queue, unqueue, when, note)]
struct General;

#[group]
#[commands(list, remove)]
struct Owner;


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
async fn note(ctx: &Context, msg: &Message) -> CommandResult {
    say(ctx, msg, "Not implemented").await;
    return Ok(());
}

#[command]
async fn queue(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // Check if discord id exists
    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap().lock().await;
  
    if !is_user_queued(msg.author.id, &db) {
        say(ctx, msg, MSG_QUEUE_ALREADY).await;
        return Ok(());
    }

    let mut note = String::from("");
    if args.len() > 0 {
        note = args_to_string(args);
    }

    // Insert into the database
    {
        let since_the_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    
        let rows_affected = db.execute(STMT_QUEUE_UP, 
                                       &[
                                          &msg.author.id.to_string(),
                                          &msg.author.name,
                                          &note,
                                          &since_the_epoch.as_millis().to_string(), 
                                       ]).unwrap();
        if rows_affected == 0 {
            say(ctx, msg, MSG_ERROR).await;
            return Ok(());
        }
    }

    //Get the queue number
    {
        let mut queue_length: u32 = 0;
        {
            let mut stmt = db.prepare(STMT_QUEUE_COUNT).unwrap();
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

    let rows_affected = db.execute(STMT_UNQUEUE, 
                                   &[
                                       &msg.author.id.to_string()
                                   ]).unwrap();

    if rows_affected > 0 {
        say(ctx, msg, MSG_REMOVE_QUEUE_SUCCESS).await;
    } else {
        say(ctx, msg, MSG_NOT_IN_QUEUE).await;
    }
    return Ok(());
}

#[command]
async fn when(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap().lock().await;

    if !is_user_queued(msg.author.id, &db) {
        say(ctx, msg, MSG_NOT_IN_QUEUE).await;
        return Ok(());
    }

    let mut queue_number_opt: Option<u32> = None;                           
    {                                                                       
        let mut stmt = db.prepare(STMT_QUEUE_NUMBER).unwrap();                          
        let mut rows = stmt.query(&[&msg.author.id.to_string()]).unwrap();  
        if let Some(row) = rows.next().unwrap() {                           
            queue_number_opt = Some(row.get(0).unwrap());                   
        }                                                                   
    }                                                                       

    if let Some(queue_number) = queue_number_opt {    
        say(ctx, msg, format!("Your queue number is: **{}**", queue_number)).await;
    } else {
        say(ctx, msg, MSG_ERROR).await;
    }


    return Ok(());
}


// Commands only allowed by mei
struct QueueEntry {
    discord_id: String,
    name: String,
    note: String,
    created: String,
}

#[command]
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    if !is_owner(&ctx, &msg).await {
        return Ok(());
    }
    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap().lock().await;
    let mut entries: Vec<QueueEntry> = Vec::new(); 
    {
        let mut stmt = db.prepare(STMT_LIST)?;
        let rows = stmt.query_map(NO_PARAMS , |row| {
            Ok(QueueEntry {
                discord_id: row.get(0).unwrap(),
                name: row.get(1).unwrap(),
                note: row.get(2).unwrap(),
                created: row.get(3).unwrap(),
            })
        })?;
        for row in rows {
            entries.push(row?);
        }
    }
   
    if entries.len() == 0 {
        say(ctx, msg, MSG_EMPTY_LIST).await;
        return Ok(());
    }

    let mut reply: String = String::from("```");
    {
        let mut buffer: String = String::new();
        for entry in entries {
            buffer.push_str(entry.discord_id.as_str());
            buffer.push('\t');
            buffer.push_str(entry.created.as_str());
            buffer.push('\t');
            buffer.push_str(entry.name.as_str());
            buffer.push('\t');
            buffer.push_str(entry.note.as_str());
            buffer.push('\n');

            if reply.len() + buffer.len() < DISCORD_MSG_LIMIT {
                reply.push_str(buffer.as_str());
                buffer.clear();
                println!("{}", buffer);
            } else {
                break;
            }
        }
    }    

    reply.push_str("```");
    say(ctx, msg, reply).await;

    return Ok(());
}

#[command]
async fn remove(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if !is_owner(&ctx, &msg).await {
        return Ok(());
    }
    if args.len() == 0 {
        say(ctx, msg, MSG_MISSING_DISCORD_ID).await;
        return Ok(());
    }

    let discord_id: UserId;
    match args.single::<UserId>() {
        Ok(v) => discord_id = v,
        Err(_) => {
            say(ctx, msg, MSG_INVALID_USER_ID).await;
            return Ok(());
        }
    }
 
    let discord_id_str = discord_id.as_u64().to_string();
    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap().lock().await;

    let rows_affected = db.execute(STMT_REMOVE_ENTRY, &[&discord_id_str]).unwrap();
    if rows_affected == 0 {
        say(ctx, msg, MSG_DISCORD_ID_NOT_EXIST).await; 
        return Ok(());
    }

    say(ctx, msg, format!("Removed {}", discord_id_str)).await;



    return Ok(());
}


#[derive(Deserialize)]
struct Config {
    token: String,
    prefix: String,
    db_path: String,
    owner_id: u64,
}

struct Handler; 
#[async_trait] impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        ctx.set_activity(Activity::playing("type !sensei help")).await;
    }
}

async fn is_owner(ctx: &Context, msg: &Message) -> bool {
    let data = ctx.data.read().await;
    let owner_id = data.get::<OwnerId>().unwrap();
    
    return msg.author.id == *owner_id;
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
            .group(&GENERAL_GROUP)
            .group(&OWNER_GROUP);
    
        
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
            data.insert::<OwnerId>(UserId(config.owner_id));        

        }
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
