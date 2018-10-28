#[macro_use]
extern crate clap;
extern crate blockchain_gateway_lib;
extern crate env_logger;

use clap::App;

fn main() {
    env_logger::init();

    let yaml = load_yaml!("cli.yml");
    let mut app = App::from_yaml(yaml);
    let matches = app.clone().get_matches();

    if let Some(_) = matches.subcommand_matches("config") {
        blockchain_gateway_lib::print_config();
    } else if let Some(_) = matches.subcommand_matches("server") {
        blockchain_gateway_lib::start_server();
    } else if let Some(matches) = matches.subcommand_matches("get_btc_transaction") {
        let hash = matches.value_of("hash").unwrap();
        blockchain_gateway_lib::get_btc_transaction(&hash);
    } else if let Some(matches) = matches.subcommand_matches("get_btc_block") {
        let hash = matches.value_of("hash").unwrap();
        blockchain_gateway_lib::get_btc_block(&hash);
    } else if let Some(matches) = matches.subcommand_matches("get_btc_last_blocks") {
        let param = matches.value_of("number").unwrap();
        let number: u64 = param.parse().unwrap();
        blockchain_gateway_lib::get_btc_last_blocks(number);
    } else if let Some(matches) = matches.subcommand_matches("get_btc_last_transactions") {
        let param = matches.value_of("number").unwrap();
        let number: u64 = param.parse().unwrap();
        blockchain_gateway_lib::get_btc_last_transactions(number);
    } else if let Some(matches) = matches.subcommand_matches("publish_btc_transactions") {
        let param = matches.value_of("number").unwrap_or("1");
        let number: u64 = param.parse().unwrap();
        let hash = matches.value_of("hash").map(|x| x.to_string());
        blockchain_gateway_lib::publish_btc_transactions(hash, number);
    } else {
        let _ = app.print_help();
        println!("\n")
    }
}
