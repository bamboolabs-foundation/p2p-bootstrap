#[derive(libp2p::swarm::NetworkBehaviour)]
struct BootstrapBehaviour {
    autonat: libp2p::autonat::Behaviour,
    identify: libp2p::identify::Behaviour,
    kademlia: libp2p::kad::Behaviour<libp2p::kad::store::MemoryStore>,
    ping: libp2p::ping::Behaviour,
    relay: libp2p::relay::Behaviour,
}

impl BootstrapBehaviour {
    const PERMANENT_BOOTNODE_DNS: &'static str = "/dnsaddr/bootstrap.libp2p.io";
    const PERMANENT_BOOTNODE_IDENTITIES: [&'static str; 4] = [
        "QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
        "QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
        "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
        "QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    ];
    const PROTOCOL_NAME: &'static str = "/ipsn/1.0.0";

    fn create(keypair: &libp2p::identity::Keypair) -> Self {
        let public_key = keypair.public();
        let peer_id = public_key.to_peer_id();

        crate::info!("Our PeerId is {peer_id}");

        let autonat = libp2p::autonat::Behaviour::new(peer_id, core::default::Default::default());
        let identify = {
            let identify_config = libp2p::identify::Config::new(Self::PROTOCOL_NAME.into(), public_key.clone())
                .with_push_listen_addr_updates(true);

            libp2p::identify::Behaviour::new(identify_config)
        };
        let kademlia = {
            let bootnode_dns =
                <libp2p::Multiaddr as core::str::FromStr>::from_str(Self::PERMANENT_BOOTNODE_DNS).unwrap();
            let kademlia_store = libp2p::kad::store::MemoryStore::new(peer_id);
            let mut kademlia = libp2p::kad::Behaviour::new(peer_id, kademlia_store);

            for id in Self::PERMANENT_BOOTNODE_IDENTITIES {
                let peer_id = <libp2p::PeerId as core::str::FromStr>::from_str(id).unwrap();

                kademlia.add_address(&peer_id, bootnode_dns.clone());
            }

            kademlia.bootstrap().unwrap();

            kademlia
        };
        let ping = libp2p::ping::Behaviour::new(core::default::Default::default());
        let relay = libp2p::relay::Behaviour::new(peer_id, Default::default());

        Self {
            autonat,
            identify,
            kademlia,
            ping,
            relay,
        }
    }
}

pub(crate) struct SwarmService {
    inner: libp2p::swarm::Swarm<BootstrapBehaviour>,
}

impl SwarmService {
    const BOOTSTRAP_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(5 * 60);

    pub(crate) fn create(keypair: libp2p::identity::Keypair, port: u16) -> crate::Result<Self> {
        let mut inner = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::new().nodelay(true).port_reuse(true),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::server,
            )?
            .with_quic()
            .with_dns()?
            .with_behaviour(BootstrapBehaviour::create)?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(tokio::time::Duration::from_secs(u64::MAX)))
            .build();
        let addr_ipv4_all =
            libp2p::Multiaddr::empty().with(libp2p::multiaddr::Protocol::Ip4(std::net::Ipv4Addr::UNSPECIFIED));
        let addr_tcp = addr_ipv4_all.clone().with(libp2p::multiaddr::Protocol::Tcp(port));
        let addr_udp = addr_ipv4_all
            .with(libp2p::multiaddr::Protocol::Udp(port))
            .with(libp2p::multiaddr::Protocol::QuicV1);
        inner.listen_on(addr_tcp)?;
        inner.listen_on(addr_udp)?;

        crate::Result::Ok(Self {
            inner,
        })
    }

    pub(crate) async fn run(mut self) -> crate::Result<()> {
        let mut bootstrap_timer = futures_timer::Delay::new(Self::BOOTSTRAP_INTERVAL);

        loop {
            if let std::task::Poll::Ready(()) = futures::poll!(&mut bootstrap_timer) {
                bootstrap_timer.reset(Self::BOOTSTRAP_INTERVAL);
                let network_info = self.inner.network_info();
                crate::info!("(Re)bootstrapping Swarm with {network_info:#?}");
                let _ = self.inner.behaviour_mut().kademlia.bootstrap();
            }

            let next_event = futures::stream::StreamExt::select_next_some(&mut self.inner).await;

            match next_event {
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Autonat(
                    libp2p::autonat::Event::OutboundProbe(libp2p::autonat::OutboundProbeEvent::Error {
                        peer,
                        error,
                        ..
                    }),
                )) => {
                    crate::error!(
                        "AutoNAT failed for {} with error: {error:?}",
                        peer.map_or("unknown".to_string(), |p| p.to_string())
                    );
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Autonat(
                    libp2p::autonat::Event::OutboundProbe(libp2p::autonat::OutboundProbeEvent::Request {
                        peer, ..
                    }),
                )) => {
                    crate::info!("AutoNAT probing request received for {peer}");
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Autonat(
                    libp2p::autonat::Event::OutboundProbe(libp2p::autonat::OutboundProbeEvent::Response {
                        peer,
                        address,
                        ..
                    }),
                )) => {
                    crate::info!("AutoNAT probing success for {peer} via: {address}");
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Identify(
                    libp2p::identify::Event::Received {
                        peer_id,
                        info,
                    },
                )) => {
                    let libp2p::identify::Info {
                        listen_addrs,
                        protocols,
                        ..
                    } = info;

                    if protocols.iter().any(|proto| proto == &libp2p::kad::PROTOCOL_NAME) {
                        for addr in listen_addrs {
                            self.inner.behaviour_mut().kademlia.add_address(&peer_id, addr);
                        }
                    }
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Kademlia(
                    libp2p::kad::Event::OutboundQueryProgressed {
                        id,
                        result,
                        ..
                    },
                )) => {
                    crate::info!("Kademlia {id:?} => {result:?}");
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                    libp2p::relay::Event::CircuitClosed {
                        src_peer_id,
                        dst_peer_id,
                        error,
                    },
                )) => {
                    if let Some(error) = error {
                        crate::error!("P2P circuit closed ({src_peer_id} => {dst_peer_id}) with {error:#?}");
                    } else {
                        crate::info!("P2P circuit closed ({src_peer_id} => {dst_peer_id})");
                    }
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                    libp2p::relay::Event::CircuitReqAccepted {
                        src_peer_id,
                        dst_peer_id,
                    },
                )) => {
                    crate::info!("P2P circuit requested ({src_peer_id} => {dst_peer_id})")
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                    libp2p::relay::Event::CircuitReqDenied {
                        src_peer_id,
                        dst_peer_id,
                    },
                )) => {
                    crate::warn!("P2P circuit denied ({src_peer_id} => {dst_peer_id})");
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                    libp2p::relay::Event::ReservationReqAccepted {
                        src_peer_id,
                        renewed,
                    },
                )) => {
                    if renewed {
                        crate::info!("P2P circuit reservation renewed ({src_peer_id})");
                    } else {
                        crate::info!("P2P circuit reservation accepted ({src_peer_id})");
                    }
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                    libp2p::relay::Event::ReservationReqDenied {
                        src_peer_id,
                    },
                )) => {
                    crate::warn!("P2P circuit reservation denied ({src_peer_id})");
                }
                libp2p::swarm::SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                    libp2p::relay::Event::ReservationTimedOut {
                        src_peer_id,
                    },
                )) => {
                    crate::warn!("P2P circuit reservation has no renewal ({src_peer_id})");
                }
                other_event => crate::debug!("{other_event:?}"),
            }
        }
    }
}
