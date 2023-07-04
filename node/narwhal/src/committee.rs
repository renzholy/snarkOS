// Copyright (C) 2019-2023 Aleo Systems Inc.
// This file is part of the snarkOS library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::helpers::PrimarySender;
use snarkvm::console::{prelude::*, types::Address};

use parking_lot::RwLock;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::sync::OnceCell;

pub struct Committee<N: Network> {
    /// A map of `address` to `stake`.
    committee: RwLock<HashMap<Address<N>, u64>>,
    /// The current round number.
    round: AtomicU64,
    /// A map of `peer IP` to `address`.
    peer_addresses: RwLock<HashMap<SocketAddr, Address<N>>>,
    /// A map of `address` to `peer IP`.
    address_peers: RwLock<HashMap<Address<N>, SocketAddr>>,
    /// The primary sender.
    primary_sender: Arc<OnceCell<PrimarySender<N>>>,
}

impl<N: Network> Committee<N> {
    /// Initializes a new `Committee` instance.
    pub fn new(round: u64) -> Self {
        Self {
            committee: Default::default(),
            round: AtomicU64::new(round),
            peer_addresses: Default::default(),
            address_peers: Default::default(),
            primary_sender: Default::default(),
        }
    }
}

impl<N: Network> Committee<N> {
    /// Returns the current round number.
    pub fn round(&self) -> u64 {
        self.round.load(Ordering::Relaxed)
    }

    /// Increments the round number.
    pub fn increment_round(&self) {
        self.round.fetch_add(1, Ordering::Relaxed);
    }
}

impl<N: Network> Committee<N> {
    /// Adds a validator to the committee.
    pub fn add_validator(&self, address: Address<N>, stake: u64) -> Result<()> {
        // Check if the validator is already in the committee.
        if self.is_committee_member(address) {
            bail!("Validator already in committee");
        }
        // Add the validator to the committee.
        self.committee.write().insert(address, stake);
        Ok(())
    }

    /// Returns the committee.
    pub fn committee(&self) -> &RwLock<HashMap<Address<N>, u64>> {
        &self.committee
    }

    /// Returns the number of validators in the committee.
    pub fn committee_size(&self) -> usize {
        self.committee.read().len()
    }

    /// Returns `true` if the given address is in the committee.
    pub fn is_committee_member(&self, address: Address<N>) -> bool {
        self.committee.read().contains_key(&address)
    }

    /// Returns the amount of stake for the given address.
    pub fn get_stake(&self, address: Address<N>) -> u64 {
        self.committee.read().get(&address).copied().unwrap_or_default()
    }

    /// Returns the total amount of stake in the committee.
    pub fn total_stake(&self) -> Result<u64> {
        // Compute the total power of the committee.
        let mut power = 0u64;
        for stake in self.committee.read().values() {
            // Accumulate the stake, checking for overflow.
            power = match power.checked_add(*stake) {
                Some(power) => power,
                None => bail!("Failed to calculate total stake - overflow detected"),
            };
        }
        Ok(power)
    }

    /// Returns the amount of stake required to reach a quorum threshold `(2f + 1)`.
    pub fn quorum_threshold(&self) -> Result<u64> {
        // Assuming `N = 3f + 1 + k`, where `0 <= k < 3`,
        // then `(2N + 3) / 3 = 2f + 1 + (2k + 2)/3 = 2f + 1 + k = N - f`.
        Ok(self.total_stake()?.saturating_mul(2) / 3 + 1)
    }

    /// Returns the amount of stake required to reach the availability threshold `(f + 1)`.
    pub fn availability_threshold(&self) -> Result<u64> {
        // Assuming `N = 3f + 1 + k`, where `0 <= k < 3`,
        // then `(N + 2) / 3 = f + 1 + k/3 = f + 1`.
        Ok(self.total_stake()?.saturating_add(2) / 3)
    }
}

impl<N: Network> Committee<N> {
    /// Returns the peer IP for the given address.
    pub fn get_peer_ip(&self, address: Address<N>) -> Option<SocketAddr> {
        self.address_peers.read().get(&address).copied()
    }

    /// Returns the address for the given peer IP.
    pub fn get_address(&self, peer_ip: SocketAddr) -> Option<Address<N>> {
        self.peer_addresses.read().get(&peer_ip).copied()
    }

    /// Inserts the given peer.
    pub(crate) fn insert_peer(&self, peer_ip: SocketAddr, address: Address<N>) {
        self.peer_addresses.write().insert(peer_ip, address);
        self.address_peers.write().insert(address, peer_ip);
    }

    /// Removes the given peer.
    pub(crate) fn remove_peer(&self, peer_ip: SocketAddr) {
        if let Some(address) = self.peer_addresses.write().remove(&peer_ip) {
            self.address_peers.write().remove(&address);
        }
    }
}

impl<N: Network> Committee<N> {
    /// Returns the primary sender.
    pub fn primary_sender(&self) -> &PrimarySender<N> {
        self.primary_sender.get().expect("Primary sender not set")
    }

    /// Sets the primary sender.
    pub fn set_primary_sender(&self, primary_sender: PrimarySender<N>) {
        self.primary_sender.set(primary_sender).expect("Primary sender already set");
    }
}
