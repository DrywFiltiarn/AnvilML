use anvilml_server::ApiDoc;
use std::fs;
use utoipa::OpenApi;

fn main() {
    let openapi_json = ApiDoc::openapi()
        .to_pretty_json()
        .expect("failed to serialize OpenAPI spec");

    fs::write("api/openapi.json", &openapi_json).expect("failed to write api/openapi.json");

    println!("Generated api/openapi.json ({} bytes)", openapi_json.len());
}
