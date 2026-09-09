use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::net::IpAddr;

pub fn register(port: u16) -> anyhow::Result<ServiceDaemon> {
    let daemon = ServiceDaemon::new()?;

    let lan: Vec<String> = local_ips().iter().map(|ip| ip.to_string()).collect();
    let mut addrs: Vec<&str> = vec!["127.0.0.1"];
    addrs.extend(lan.iter().map(|s| s.as_str()));

    let info = ServiceInfo::new(
        "_aa._tcp.local.",
        "AAgent Server",
        "aa-server.local.",
        &addrs as &[&str],
        port,
        &[("txtvers", "1")] as &[(&str, &str)],
    )?;

    daemon.register(info)?;
    tracing::info!("mDNS: registered _aa._tcp.local on port {port}");
    Ok(daemon)
}

fn local_ips() -> Vec<IpAddr> {
    let mut ips = Vec::new();
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) == 0 {
            let mut ptr = ifap;
            while !ptr.is_null() {
                let ifa = &*ptr;
                if !ifa.ifa_addr.is_null()
                    && (*ifa.ifa_addr).sa_family as i32 == libc::AF_INET
                {
                    let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                    let ip = std::net::Ipv4Addr::from(sin.sin_addr.s_addr.to_ne_bytes());
                    if !ip.is_loopback() && !ip.is_link_local() {
                        ips.push(IpAddr::V4(ip));
                    }
                }
                ptr = ifa.ifa_next;
            }
            libc::freeifaddrs(ifap);
        }
    }
    ips
}
