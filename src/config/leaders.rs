use anyhow::Result;

use crate::validate::normalize_address;

use super::{AccountConfig, AppConfig, LeaderConfig};

impl AppConfig {
    pub fn add_leader(&mut self, mut leader: LeaderConfig) -> Result<()> {
        leader.address = normalize_address(&leader.address)?;
        if self
            .leaders
            .iter()
            .any(|existing| existing.address.eq_ignore_ascii_case(&leader.address))
        {
            anyhow::bail!("leader already exists: {}", leader.address);
        }
        self.leaders.push(leader);
        Ok(())
    }

    pub fn remove_leader(&mut self, address: &str) -> Result<LeaderConfig> {
        let address = normalize_address(address)?;
        let index = self
            .leaders
            .iter()
            .position(|leader| leader.address.eq_ignore_ascii_case(&address))
            .ok_or_else(|| anyhow::anyhow!("leader not found: {address}"))?;
        Ok(self.leaders.remove(index))
    }

    pub fn leader_mut(&mut self, address: &str) -> Result<&mut LeaderConfig> {
        let address = normalize_address(address)?;
        self.leaders
            .iter_mut()
            .find(|leader| leader.address.eq_ignore_ascii_case(&address))
            .ok_or_else(|| anyhow::anyhow!("leader not found: {address}"))
    }

    pub fn account_for_leader(&self, leader: &LeaderConfig) -> Result<&AccountConfig> {
        let Some(account_name) = leader.account_name.as_deref() else {
            return Ok(&self.account);
        };
        if self.account.name == account_name {
            return Ok(&self.account);
        }
        self.accounts
            .iter()
            .find(|account| account.name == account_name)
            .ok_or_else(|| anyhow::anyhow!("unknown account_name: {account_name}"))
    }
}
