#[derive(core::fmt::Debug)]
#[derive(clap::Parser)]
#[clap(author, version)]
pub(crate) struct CLIArgs {
    /// Join global IPFS network
    #[clap(long, short, action, default_value_t = false)]
    pub(crate) join_ipfs: bool,
    /// TCP & UDP Port
    #[clap(long, short, default_value_t = 4011)]
    pub(crate) port: u16,
    /// Ed25519 Secret Key Bytes in hexadecimal
    #[clap(long, short, value_parser = CLIArgs::decode_secret_seed, default_value = CLIArgs::SECRET_KEY_RANDOM_TRIGGER)]
    pub(crate) secret_key: libp2p::identity::Keypair,
}

impl CLIArgs {
    const SECRET_KEY_RANDOM_TRIGGER: &'static str = "{RANDOM}";

    fn decode_secret_seed(input: &str) -> crate::Result<libp2p::identity::Keypair> {
        let ed25519_keypair = if input.eq(Self::SECRET_KEY_RANDOM_TRIGGER) {
            libp2p::identity::Keypair::generate_ed25519()
        } else {
            let secret_seed = hex::decode(input)?;

            libp2p::identity::Keypair::ed25519_from_bytes(secret_seed)?
        };

        crate::Result::Ok(ed25519_keypair)
    }
}
