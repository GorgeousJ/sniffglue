extern crate pcap;
#[macro_use] extern crate nom;
extern crate pktparse;
extern crate dns_parser;
extern crate tls_parser;
extern crate dhcp4r;
extern crate ansi_term;
extern crate threadpool;
extern crate num_cpus;
extern crate reduce;
extern crate clap;

use pcap::Device;
use pcap::Capture;

use threadpool::ThreadPool;

use std::thread;
use std::sync::mpsc;

mod centrifuge;
mod fmt;
mod structs;
mod nom_http;

use clap::{App, Arg};


type Message = structs::raw::Raw;
type Sender = mpsc::Sender<Message>;
type Receiver = mpsc::Receiver<Message>;


fn main() {
    let matches = App::new("sniffglue")
        .version("0.1.0")
        .arg(Arg::with_name("promisc")
            .short("p")
            .long("promisc")
            .help("Set device to promisc")
        )
        .arg(Arg::with_name("detailed")
            .short("d")
            .long("detailed")
            .help("Detailed output")
        )
        .arg(Arg::with_name("noisy")
            .short("x")
            .long("noisy")
            .help("Log noisy packets")
        )
        .arg(Arg::with_name("dev")
            .help("Device for sniffing")
        )
        .get_matches();

    let dev = match matches.value_of("dev") {
        Some(dev) => dev.to_owned(),
        None => Device::lookup().unwrap().name,
    };
    let log_noise = matches.occurrences_of("noisy") > 0;
    let promisc = matches.occurrences_of("promisc") > 0;

    let layout = match matches.occurrences_of("detailed") {
        0 => fmt::Layout::Compact,
        _ => fmt::Layout::Detailed,
    };

    let config = fmt::Config::new(layout, log_noise);

    eprintln!("Listening on device: {:?}", dev);
    let mut cap = Capture::from_device(dev.as_str()).unwrap()
                    .promisc(promisc)
                    .open().unwrap();

    let (tx, rx): (Sender, Receiver) = mpsc::channel();
    let filter = config.filter();

    let join = thread::spawn(move || {
        let cpus = num_cpus::get();
        let pool = ThreadPool::new(cpus);

        while let Ok(packet) = cap.next() {
            // let ts = packet.header.ts;
            // let len = packet.header.len;

            let tx = tx.clone();
            let packet = packet.data.to_vec();

            let filter = filter.clone();
            pool.execute(move || {
                match centrifuge::parse(&packet) {
                    Ok(packet) => {
                        if filter.matches(&packet) {
                            tx.send(packet).unwrap()
                        }
                    }
                    Err(_) => (),
                };
            });
        }
    });

    let format = config.format();
    for packet in rx.iter() {
        format.print(packet);
    }

    join.join().unwrap();
}