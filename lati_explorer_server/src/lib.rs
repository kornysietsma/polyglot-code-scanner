#![warn(clippy::all)]
extern crate failure;
#[macro_use]
extern crate log;
extern crate failure_tools;
extern crate iron;
extern crate mount;
extern crate staticfile;

use failure::Error;

use std::path::Path;

use iron::mime::Mime;
use iron::prelude::*;
use iron::status;
use iron::Handler;
use iron::Iron;
use mount::Mount;
use staticfile::Static;

struct JsonPayload {
    json: String,
}

impl Handler for JsonPayload {
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        info!("Serving JSON file data");
        let content_type = "application/json".parse::<Mime>().unwrap();

        Ok(Response::with((
            content_type,
            status::Ok,
            self.json.clone(),
        ))) // TODO: why clone?
    }
}

pub fn serve(explorer_files: &Path, json_data: &str) -> Result<(), Error> {
    let mut mount = Mount::new();

    // Serve the shared JS/CSS at /
    mount.mount("/", Static::new(explorer_files));
    mount.mount(
        "/js/data/flare.json",
        JsonPayload {
            json: json_data.to_owned(),
        },
    );

    eprintln!("Lati server running on http://localhost:3000/");

    Iron::new(mount).http("localhost:3000")?;

    Ok(())
}
