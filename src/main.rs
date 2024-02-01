use chrono::Utc;
use infobip_sdk::api::whatsapp::WhatsappClient;
use infobip_sdk::configuration::Configuration;
use infobip_sdk::model::whatsapp::{SendTextRequestBody, TextContent};
use std::env;
use sysinfo::System;
use tokio::time::Duration;
use tokio::{join, time};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// CPU usage percentage threshold
    #[arg(default_value_t = 90.0)]
    cpu_usage_threshold: f32,

    /// Memory usage percentage threshold
    #[arg(default_value_t = 80)]
    mem_usage_threshold: u64,

    /// Anomalous cycles needed to trigger alert
    #[arg(default_value_t = 20)]
    cycles_for_alert: usize,

    /// OK cycles required between alerts
    #[arg(default_value_t = 20)]
    cycles_between_alert: usize,

    /// Refresh interval in seconds
    #[arg(default_value_t = 2)]
    refresh_interval_secs: u64
}

async fn send_alert(message: String) {
    let client = WhatsappClient::with_configuration(Configuration::from_env_api_key().unwrap());

    let request_body = SendTextRequestBody::new(
        env::var("WA_SENDER").unwrap().as_str(),
        env::var("WA_DESTINATION").unwrap().as_str(),
        TextContent::new(message.as_str()),
    );

    let response = client.send_text(request_body).await.unwrap();

    println!("Alert: {message} => HTTP response: {:?}", response.status);
}

fn print_system_stats(sys: &System) {
    println!(
        "System:     {} {}",
        System::name().unwrap(),
        System::kernel_version().unwrap()
    );
    println!("OS version: {}", System::os_version().unwrap());
    println!("Host name:  {}", System::host_name().unwrap());
    println!("CPUs:       {}", sys.cpus().len());
    println!("Memory:     {} GiB", sys.total_memory() / bytesize::GIB);
    println!("Swap:       {} GiB", sys.total_swap() / bytesize::GIB);
}

async fn check_anomalies(mut sys: System, args: Args) {
    let mut interval = time::interval(Duration::from_secs(args.refresh_interval_secs));

    // Control counters to avoid alerting every iteration or short spikes.
    let mut cpus_high_cycles = vec![0; sys.cpus().len()];
    let mut cpus_ok_cycles = vec![args.cycles_between_alert; sys.cpus().len()];
    let mut mem_high_cycles = 0usize;
    let mut mem_ok_cycles = args.cycles_between_alert;

    loop {
        // Refresh CPU for accuracy.
        sys.refresh_cpu();

        interval.tick().await;

        sys.refresh_all();

        let ts = Utc::now().format("%m-%d-%y %T UTC").to_string();
        let hostname = System::host_name().unwrap();

        let mut handles = vec![];

        // Check CPU usages.
        for (i, cpu) in sys.cpus().iter().enumerate() {
            let usage = cpu.cpu_usage();
            if usage > args.cpu_usage_threshold {
                cpus_high_cycles[i] += 1;
                if cpus_high_cycles[i] >= args.cycles_for_alert
                    && cpus_ok_cycles[i] >= args.cycles_between_alert
                {
                    cpus_ok_cycles[i] = 0;
                    handles.push(send_alert(format!(
                        "{ts} {hostname}: High CPU{i} usage: {usage:.1}%",
                    )));
                }
            } else {
                cpus_high_cycles[i] = 0;
                cpus_ok_cycles[i] += 1;
            }
        }

        // Check available memory.
        if sys.used_memory() > sys.total_memory() / 100 * args.mem_usage_threshold {
            mem_high_cycles += 1;
            if mem_high_cycles >= args.cycles_for_alert && mem_ok_cycles >= args.cycles_between_alert {
                mem_ok_cycles = 0;
                handles.push(send_alert(format!(
                    "{ts} {hostname}: High memory usage: >{:.1}%", args.mem_usage_threshold
                )));
            }
        } else {
            mem_ok_cycles += 1;
        }

        // Send the alerts.
        for handle in handles {
            join!(handle);
        }
    }
}

#[tokio::main]
async fn main() {
    let args= Args::parse();

    println!("=====================================");
    println!("Infobip WhatsApp Rusty System Monitor");
    println!("=====================================");

    let sys = System::new_all();

    print_system_stats(&sys);

    println!("\nChecking for system anomalies ...");
    check_anomalies(sys, args).await
}
