use infobip_sdk::api::whatsapp::WhatsappClient;
use infobip_sdk::configuration::Configuration;
use infobip_sdk::model::whatsapp::{SendTextRequestBody, TextContent};
use std::env;
use sysinfo::{Disks, System};
use tokio::join;
use tokio::time::{sleep, Duration};

async fn send_alert(message: String) {
    let client = WhatsappClient::with_configuration(Configuration::from_env_api_key().unwrap());

    let request_body = SendTextRequestBody::new(
        env::var("WA_SENDER").unwrap().as_str(),
        env::var("WA_DESTINATION").unwrap().as_str(),
        TextContent::new(message.as_str()),
    );

    let response = client.send_text(request_body).await.unwrap();

    println!("WhatsApp response: {:?}", response.status);
}

async fn check_anomalies(sys: &System, disks: &Disks) {
    let mut handles = vec![];
    let hostname = System::host_name().unwrap();

    for (i, cpu) in sys.cpus().iter().enumerate() {
        if cpu.cpu_usage() > 90.0 {
            handles.push(send_alert(format!("{}: CPU_{} usage too high.", hostname, i)));
        }
    }

    if sys.used_memory() > sys.total_memory() / 100 * 80 {
        handles.push(send_alert(format!("{}: Memory usage too high. > 80%", hostname)));
    }

    for handle in handles {
        join!(handle);
    }

    // for (pid, process) in sys.processes() {
    //     println!("[{pid}] {} {:?}", process.name(), process.memory());
    // }

    // // We display all disks' information:
    // println!("=> disks:");
    // for disk in disks {
    //     println!(
    //         "Disk: {}, {} GB available",
    //         disk.name().to_str().unwrap(),
    //         disk.available_space() / 1024 / 1024 / 1024
    //     );
    // }
}

#[tokio::main]
async fn main() {
    let mut sys = System::new_all();
    let mut disks = Disks::new_with_refreshed_list();

    println!("Infobip Rusty System Monitor");
    println!("============================");
    println!("System:     {:?} {:?}", System::name().unwrap(), System::kernel_version().unwrap());
    println!("OS version: {:?}", System::os_version().unwrap());
    println!("Host name:  {:?}", System::host_name().unwrap());
    println!("CPUs:       {}", sys.cpus().len());
    println!("Memory:     {} Mbytes", sys.total_memory() / 1024 / 1024);
    println!("Swap:       {} Mbytes", sys.total_swap() / 1024 / 1024);

    println!("\nChecking for system anomalies ...");
    loop {
        sys.refresh_all();
        sleep(Duration::from_secs(3)).await;
        sys.refresh_all();
        disks.refresh();

        check_anomalies(&sys, &disks).await;
    }
}
