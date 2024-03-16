use crate::db::Database;
use crate::routes::ApiError;
use crate::routes::*;
use crate::utils;

use mysql::{params, prelude::Queryable, Row};
use rand_core::{RngCore, OsRng};
use warp::reject::Rejection;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::Serialize;

const SESSION_REMEMBER_EXPIRE_HOUR: u64 = 24*7;
const SESSION_NO_REMEMBER_EXPIRE_MINUTE: u64 = 30;
const REFRESHED_SESSION_EXPIRE_HOUR: u64 = 24*7;

// response format
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize)]
pub struct ResponseData {
    session: String,
    refresh_token: String,
}

pub async fn login(json_data: LoginData, database: Database) -> Result<impl warp::Reply, Rejection> {
    let username: String = json_data.clone().username;
    let pw = json_data.pw;

    // get salt and pw from login table
    let mut conn = database.pool.get_conn().unwrap();
    let result: Vec<Row> = conn.exec("SELECT id, salt, pw FROM login WHERE username = :username", params! {"username" => username.clone()}).unwrap();
    if result.len() == 0 {
        return Err(warp::reject::custom(ApiError::NotAuthorized))
    }

    // check if user pw is correct
    let (id, salt, db_pw): (u64, String, String) = mysql::from_row(result[0].clone());
    let hashed_pw = utils::hash_from_string(format!("{}{}", pw, salt));
    if hashed_pw != db_pw {
        return Err(warp::reject::custom(ApiError::NotAuthorized))
    }

    // check if user has expired session
    let result: Vec<Row> = conn.exec("SELECT session, expire FROM session WHERE id = :id", params! {"id" => id.clone()}).unwrap();
    if result.len() > 0 {
        for row in result {
            let (session, expire): (String, u64) = mysql::from_row(row);
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            if current_time > expire {
                // delete expired session
                let _result: Vec<Row> = conn.exec("DELETE FROM session WHERE session = :session", params! {"session" => session}).unwrap();
            }
        }
    }

    // make session by hashing random number and id
    let mut key = OsRng.next_u64().to_le_bytes().to_vec();
    let mut session_source = id.clone().to_string().into_bytes();
    session_source.append(&mut key);
    let session = utils::hash_from_u8(session_source);
    // make expire time
    let expire = match json_data.remember {
        true => SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60*60*SESSION_REMEMBER_EXPIRE_HOUR,
        false => SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60*60*SESSION_NO_REMEMBER_EXPIRE_MINUTE,
    };
    // insert session to the session table
    let _result: Vec<Row> = conn.exec("INSERT INTO session (id, session, expire) VALUES (:id, :session, :expire)", params! {"id" => id.clone(), "session" => session.clone(), "expire" => expire}).unwrap();

    // make refresh_toke by hashing random number and id
    let mut key = OsRng.next_u64().to_le_bytes().to_vec();
    let mut refresh_token_source = id.clone().to_string().into_bytes();
    refresh_token_source.append(&mut key);
    let refresh_token = utils::hash_from_u8(refresh_token_source);
    
    // insert refresh_token to the login table
    let _result: Vec<Row> = conn.exec("UPDATE login SET refresh_token = :refresh_token WHERE id = :id", params! {"refresh_token" => refresh_token.clone(), "id" => id}).unwrap();

    // response
    let response = ResponseData {
        session: session,
        refresh_token: refresh_token,
    };

    return Ok(warp::reply::json(&response));
}

pub async fn refresh(json_data: RefreshData, database: Database) -> Result<impl warp::Reply, Rejection> {
    // check if the refresh token is valid
    let refresh_token = json_data.refresh_token;
    let mut conn = database.pool.get_conn().unwrap();
    let result: Vec<Row> = conn.exec("SELECT id FROM login WHERE refresh_token = :refresh_token", params! {"refresh_token" => refresh_token}).unwrap();
    if result.len() == 0 {
        return Err(warp::reject::custom(ApiError::NotAuthorized))
    }

    // make session by hashing random number and id
    let mut key = OsRng.next_u64().to_le_bytes().to_vec();
    let id: String = mysql::from_row(result[0].clone());
    let mut session_source = id.clone().into_bytes();
    session_source.append(&mut key);
    let session = utils::hash_from_u8(session_source);
    // make expire
    let expire = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + REFRESHED_SESSION_EXPIRE_HOUR * 3600;
    // insert new session to session table
    let _result: Vec<Row> = conn.exec("INSERT INTO session (id, session, expire) VALUES (:id, :session, :expire)", params! {"id" => id.clone(), "session" => session.clone(), "expire" => expire}).unwrap();

    // update used refresh token to new refresh token
    let mut key = OsRng.next_u64().to_le_bytes().to_vec();
    let mut refresh_token_source = id.clone().into_bytes();
    refresh_token_source.append(&mut key);
    let refresh_token = utils::hash_from_u8(refresh_token_source);
    
    let _result: Vec<Row> = conn.exec("UPDATE login SET refresh_token = :refresh_token WHERE id = :id", params! {"refresh_token" => refresh_token.clone(), "id" => id}).unwrap();

    // reponse
    let response = ResponseData {
        session: session,
        refresh_token: refresh_token,
    };
    return Ok(warp::reply::json(&response));
}
