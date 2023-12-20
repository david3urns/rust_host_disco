/*
VERSION 1.1
Host discovery tool created by David Burns. This script will accept an IP address with CIDR notation
from the user, convert it from string to Ipv4Addr and int, then iterate through all possible addresses
given the IP/CIDR combination. It prints every discovered host IP to the terminal.

TODO:
Add threading
*/

#![allow(unused_comparisons)]

use std::process::{Command, Stdio};
use std::str;
use std::io::{self, Write};
use std::net::{Ipv4Addr};

//function for validating the IP and CIDR provided by the user
fn validate_ip_cidr(input: &str) -> Result<(String, u8), String> {
    let parts: Vec<&str> = input.split('/').collect();
    if parts.len() != 2 {
        return Err("Invalid format, expected IP with CIDR prefix.".to_string());
    }

    //trim the IP from the ip/cidr combo
    let ip_address = parts[0].trim();
    if !validate_ip_address(ip_address) {
        return Err("Invalid IP address.".to_string());
    }

    //trim the cidr from the ip/cidr combo
    let cidr_prefix: u8 = match parts[1].trim().parse() {
        Ok(prefix) => prefix,
        Err(_) => return Err("Invalid CIDR prefix.".to_string()),
    };

    //checks the CIDR notation to make sure it is within range
    if cidr_prefix > 32 {
        return Err("CIDR prefix must be a number between 0 - 32.".to_string());
    }

    Ok((ip_address.to_string(), cidr_prefix))
}

//function to validate the IP address length and octet values,
fn validate_ip_address(ip_address: &str) -> bool {
    let octets: Vec<&str> = ip_address.split('.').collect();
    if octets.len() != 4 {
        return false;
    }

    for octet in octets {
        if let Ok(value) = octet.parse::<u8>() {
            if value > 255 {
                return false;
            }
        }
        else {
            return false;
        }
    }

    true
}


fn main() {
    clear_screen();
    //get user input:
    banner("Network Host Discovery");
    println!("");
    let mut ip_cidr = String::new();
    
    print!("Please enter an IP address with CIDR notation (e.g. 192.168.1.0/24): ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut ip_cidr).unwrap();
    let ip_cidr = ip_cidr.trim();
  
    //Validate IP with CIDR prefix
    let (_ip_address, _cidr_prefix) = match validate_ip_cidr(ip_cidr) {
        Ok((ip, cidr)) => (ip, cidr),
        Err(error) => {
            eprintln!("Input validation failed, {}.", error);
            return;
        }
    };
    
    //split the ip_cidr variable into two variables, one for IP, one for CIDR
    let mut parts = ip_cidr.split("/");
    let ip_address = parts.next().unwrap();
    let cidr_not = parts.next().unwrap();
    
    //parse and convert the ip address from a string into an ipv4addr that can be used
    let ip_addr_parse = ip_address.parse::<Ipv4Addr>().unwrap();
    let cidr_not_parse = cidr_not.parse().unwrap();
    let subnet_mask = !0u32.checked_shr(cidr_not_parse).unwrap_or(0);

    //convert the ip address and subnet mask to a u32
    let ip_address_u32 = u32::from(ip_addr_parse);
    let subnet_mask_u32 = u32::from(subnet_mask);

    //create a vec to store all up ip addresses:
    let mut up_ips = Vec::new();

    //create variables for tracking total versus up ip scans:
    let mut total_count = 0;
    let mut up_count = 0;
    
    //iterate through all the possible IP addresses given the provided IP/CIDR, sending
    //each possible address to the ping function above

    println!("");
    for i in 0..(1 << (32 - cidr_not_parse)) {
        let address_u32 = ip_address_u32 & subnet_mask_u32 | i;
        let address = Ipv4Addr::from(address_u32);
        let address: &str = &address.to_string();

    //start the process of pinging all the addresses
    let ping_out = Command::new("ping")     //runs the ping command
    .arg(address)                                  //provides the argument from the function as an argument to the ping command
    .arg("-c 1")                                   //adds the -c 1 argument, telling the command to only run once (ping will run until interrupted by default)
    .stdout(Stdio::piped())                   //captures the output of the ping command
    .output()
    .unwrap();

    let ping_stdout = String::from_utf8(ping_out.stdout).unwrap();
  
    total_count += 1;

    if ping_stdout.contains("1 received") {
        up_count += 1;
        println!("Ping successful, {} is \x1b[0;32mup\x1b[0m.", address);
        up_ips.push(address.to_string());
    }

    else {
        println!("Ping unsuccessful, {} is \x1b[31mdown\x1b[0m.", address);
    }
    io::stdout().flush().unwrap();
}

println!("");
banner("Results");
println!("");
//print summary of all up ip addresses:
println!("The following IP addresses were up:");
for ip in up_ips{
    println!("\x1b[0;32m{}\x1b[0m", ip);
}

//print summary of up vs total ports:
println!("");
println!("Scanned a total of {} IP addresses, of which {} were up.", total_count, up_count);

//function to create a banner for each menu
fn banner(ban_title: &str) {
    let h_border = "═";
    let v_border = "║";
    let tl_corner = "╔";   
    let tr_corner = "╗";
    let bl_corner = "╚";
    let br_corner = "╝";

    //determine the length of the title string
    let title_length = ban_title.len();

    //print the actual box:
    println!("{}{}{}{}{}", tl_corner, h_border, h_border.repeat(title_length), h_border, tr_corner);
    println!("{}{}{}{}{}", v_border, " ", ban_title, " ", v_border);
    println!("{}{}{}{}{}", bl_corner, h_border, h_border.repeat(title_length), h_border, br_corner);

    //future feature to justify and add color to the banner and text
}

fn clear_screen(){
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    }

}


