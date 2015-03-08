#![feature(io)]

extern crate cookie;
extern crate hyper;
// Freakin really? Why are dashes allowed here?
extern crate "rustc-serialize" as serialize;

use cookie::CookieJar;
use hyper::{Client, header};
use hyper::header::Headers;
use serialize::json;
use std::error::FromError;
use std::fs::File;
use std::io;
use std::io::Read;

struct Pc3Client {
    url: String,
    jar: CookieJar<'static>
}

#[derive(Debug)]
enum Pc3Error {
    Http(hyper::HttpError),
    Json(json::DecoderError),
    Io(io::Error),
    Other(&'static str)
}
impl FromError<hyper::HttpError> for Pc3Error {
    fn from_error(err: hyper::HttpError) -> Pc3Error {
        Pc3Error::Http(err)
    }
}
impl FromError<json::DecoderError> for Pc3Error {
    fn from_error(err: json::DecoderError) -> Pc3Error {
        Pc3Error::Json(err)
    }
}
impl FromError<io::Error> for Pc3Error {
    fn from_error(err: io::Error) -> Pc3Error {
        Pc3Error::Io(err)
    }
}

static BOUNDARY: &'static str = "-----------------------------------pc3client-sep";

fn create_submit_body(boundary: &str, src: &mut File) -> Result<Vec<u8>, Pc3Error> {
    let mut file_content = Vec::new();
    try!(src.read_to_end(&mut file_content));

    Ok("--".bytes()
       .chain(boundary.bytes())
       .chain("\nContent-Disposition: form-data; name=\"teamCode\"; filename=\"".bytes())
       .chain(src.path().unwrap().file_name().unwrap().to_str().unwrap().bytes())
       .chain("\"\nContent-Type: application/octet-stream\n\n".bytes())
       .chain(file_content.into_iter())
       .chain("\n--".bytes())
       .chain(boundary.bytes())
       .chain("--".bytes())
       .collect())
}

impl Pc3Client {
    fn new(url: &str) -> Pc3Client {
        Pc3Client {
            url: url.to_string(),
            jar: CookieJar::new(url.as_bytes())
        }
    }

    fn authenticate(&mut self, user: &str, pass: &str) -> Result<(), Pc3Error> {
        let mut headers = Headers::new();
        headers.set(header::ContentType("application/x-www-form-urlencoded".parse().unwrap()));

        let mut client = Client::new();
        let res = try!(client
                       .post(&self.url.chars().chain("/authenticate".chars()).collect::<String>()[..])
                       .headers(headers)
                       .body(&format!("username={}&password={}", user, pass)[..])
                       .send());
        if let Some(&header::SetCookie(ref cookies)) = res.headers.get() {
            for cookie in cookies {
                self.jar.add(cookie.clone());
            }
            Ok(())
        } else {
            Err(Pc3Error::Other("Did not receive authentication cookie"))
        }
    }
    fn compete(&self, problem_name: &str, mut src: &mut File) -> Result<Result<i32, ()>, Pc3Error> {
        if let Some(session) = self.jar.find("session") {
            let mut headers = Headers::new();
            headers.set_raw("Content-Type", vec![b"multipart/form-data; boundary=".to_vec(), BOUNDARY.bytes().collect()]);
            headers.set_raw("Cookie", vec![b"session=".to_vec(), session.value.bytes().collect()]);

            let mut client = Client::new();
            let mut res = try!(client
                               .post(&self.url.chars().chain("/compete/".chars()).chain(problem_name.chars()).chain("/".chars()).chain(src.path().unwrap().extension().unwrap().to_str().unwrap().chars()).collect::<String>()[..])
                               .headers(headers)
                               .body(&unsafe {String::from_utf8_unchecked(try!(create_submit_body(BOUNDARY, src)))}[..])
                               .send());
            let mut result = String::new();
            try!(res.read_to_string(&mut result));
            let (success, score) = try!(json::decode::<(bool, i32)>(&result));

            if success {
                Ok(Ok(score))
            } else {
                Ok(Err(()))
            }
        } else {
            // No cookie from logging in
            Err(Pc3Error::Other("Not authenticated"))
        }
    }
    fn scores(&self) -> Result<Vec<(String, i32)>, Pc3Error> {
        let mut client = Client::new();
        let mut res = try!(client
                       .get(&self.url.chars().chain("/scores".chars()).collect::<String>()[..])
                       .send());
        let mut result = String::new();
        try!(res.read_to_string(&mut result));
        Ok(try!(json::decode::<Vec<(String, i32)>>(&result)))
    }
    fn inform(&self, problem_name: &str) -> Result<String, Pc3Error> {
        let mut client = Client::new();
        let mut res = try!(client
                           .get(&self.url.chars().chain("/inform/".chars()).chain(problem_name.chars()).collect::<String>()[..])
                           .send());
        let mut result = String::new();
        try!(res.read_to_string(&mut result));
        Ok(try!(json::decode::<String>(&result)))
    }
}

fn main() {
    let mut client = Pc3Client::new("http://10.0.11.3:5000");
    client.authenticate("team1", "password").unwrap();
    println!("{}", client.inform("problem1").unwrap());
    println!("{:?}", client.compete("problem1", &mut File::open("./resources/program.java").unwrap()).unwrap());
    println!("{:?}", client.scores().unwrap());
}
