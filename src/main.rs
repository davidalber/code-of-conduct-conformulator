#![feature(proc_macro_hygiene, decl_macro)]

extern crate code_of_conduct_conformulator;
#[macro_use] extern crate rocket;
extern crate rocket_contrib;
extern crate serde_json;

use code_of_conduct_conformulator::{
    check_repository_conformance, get_org_repositories, make_expected_satellite, ConformanceReport,
};
use rocket::request::State;
use rocket::response::NamedFile;
use rocket_contrib::json::Json;
use rocket_contrib::templates::Template;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, SystemTime};

pub const ORGANIZATIONS: [&str; 3] = ["rust-lang", "rust-lang-nursery", "rust-lang-deprecated"];

struct CacheItem {
    val: String,
    created_on: SystemTime,
    ttl: Duration,
}

impl CacheItem {
    fn new(val: &str, ttl: u64) -> CacheItem {
        CacheItem {
            val: val.to_owned(),
            created_on: SystemTime::now(),
            ttl: Duration::new(ttl, 0),
        }
    }

    fn is_expired(&self) -> bool {
        SystemTime::now() > self.created_on + self.ttl
    }
}

pub struct Cacheit {
    vals: HashMap<String, CacheItem>,
}

impl Cacheit {
    pub fn new() -> Cacheit {
        Cacheit {
            vals: HashMap::new(),
        }
    }

    fn get(&self, key: &str) -> Option<String> {
        match self.vals.get(key) {
            Some(item) => match item.is_expired() {
                false => Some(item.val.clone()),
                true => None,
            },
            None => None,
        }
    }

    fn set(&mut self, key: &str, val: &str, ttl: u64) {
        self.vals.insert(key.to_owned(), CacheItem::new(val, ttl));
    }
}

fn get_all_repos() -> Vec<String> {
    let mut repos: Vec<String> = Vec::new();
    for org in ORGANIZATIONS.iter() {
        repos.append(&mut get_org_repositories(org));
    }

    repos
}

#[get("/healthcheck")]
fn healthcheck() -> Json<String> {
    Json(String::from("ok"))
}

#[get("/conduct")]
fn conduct(cacheit: State<RwLock<Cacheit>>) -> String {
    let key = "conduct";
    if let Some(r) = cacheit.read().unwrap().get(key) {
        return r;
    }

    let r = make_expected_satellite();
    cacheit.write().unwrap().set(key, &r, 5);
    r
}

fn get_conformance(cacheit: State<RwLock<Cacheit>>) -> ConformanceReport {
    let key = "conformance";
    if let Some(r) = cacheit.read().unwrap().get(key) {
        let r: ConformanceReport = serde_json::from_str(&r).unwrap();
        return r;
    }

    let repos = get_all_repos();
    let r = check_repository_conformance(&repos);
    cacheit
        .write()
        .unwrap()
        .set(key, &serde_json::to_string(&r).unwrap(), 3600);
    r
}

#[get("/conformance")]
fn conformance(cacheit: State<RwLock<Cacheit>>) -> Json<ConformanceReport> {
    Json(get_conformance(cacheit))
}

#[get("/css/<file..>")]
fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("assets/css/").join(file)).ok()
}

#[get("/")]
fn index(cacheit: State<RwLock<Cacheit>>) -> Template {
    let context = get_conformance(cacheit);
    Template::render("index", &context)
}

fn main() {
    rocket::ignite()
        .manage(RwLock::new(Cacheit::new()))
        .mount("/", routes![conduct, conformance, files, healthcheck, index])
        .attach(Template::fairing())
        .launch();
}
