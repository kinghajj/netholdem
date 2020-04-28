use crate::model::EndpointId;
use rand_core::{CryptoRng, RngCore};
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

/// The key data for a new client which has not introduced themselves yet to the
/// server.
pub struct NewClientKey {
    secret: EphemeralSecret,
    public_key: PublicKey,
}

impl NewClientKey {
    /// Create a new, random client key.
    pub fn new<R>(r: &mut R) -> Self
    where
        R: RngCore + CryptoRng,
    {
        let secret = EphemeralSecret::new(r);
        let public_key = PublicKey::from(&secret);
        NewClientKey { secret, public_key }
    }

    /// Exchange keys with the server, consuming this key data and producing
    /// a new set with the shared secret.
    pub fn exchange(self, server: &PublicKey) -> AuthKey {
        AuthKey {
            public_key: self.public_key,
            shared_secret: self.secret.diffie_hellman(server),
        }
    }
}

/// The key data for a client which has introduced themselves to the server.
/// This contains a shared secret, by which a client can authenticate themselves
/// when reconnecting.
pub struct AuthKey {
    public_key: PublicKey,
    shared_secret: SharedSecret,
}

impl<'a> From<&'a NewClientKey> for EndpointId {
    fn from(f: &'a NewClientKey) -> EndpointId {
        EndpointId(f.public_key.as_bytes().clone())
    }
}

impl<'a> From<&'a AuthKey> for EndpointId {
    fn from(f: &'a AuthKey) -> EndpointId {
        EndpointId(f.public_key.as_bytes().clone())
    }
}
