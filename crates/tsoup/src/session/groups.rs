use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt as _;
use uuid::Uuid;

use super::SessionError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
}

pub struct Groups {
    path: PathBuf,
    groups: Vec<Group>,
}

impl Groups {
    pub async fn load(path: PathBuf) -> Result<Self, SessionError> {
        let groups = match tokio::fs::read(&path).await {
            Ok(contents) => serde_json::from_slice(&contents)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.into()),
        };

        Ok(Self { path, groups })
    }

    pub fn list(&self) -> Vec<Group> {
        self.groups.clone()
    }

    pub fn contains(&self, id: Uuid) -> bool {
        self.groups.iter().any(|group| group.id == id)
    }

    pub async fn create(&mut self, name: String) -> Result<Group, SessionError> {
        let group = Group {
            id: Uuid::now_v7(),
            name,
        };
        let mut groups = self.groups.clone();
        groups.push(group.clone());
        self.store(groups).await?;
        Ok(group)
    }

    pub async fn rename(&mut self, id: Uuid, name: String) -> Result<(), SessionError> {
        let mut groups = self.groups.clone();
        groups
            .iter_mut()
            .find(|group| group.id == id)
            .ok_or(SessionError::NoSuchGroup)?
            .name = name;
        self.store(groups).await
    }

    pub async fn remove(&mut self, id: Uuid) -> Result<(), SessionError> {
        if !self.contains(id) {
            return Err(SessionError::NoSuchGroup);
        }

        let mut groups = self.groups.clone();
        groups.retain(|group| group.id != id);
        self.store(groups).await
    }

    async fn store(&mut self, groups: Vec<Group>) -> Result<(), SessionError> {
        let staging = self.path.with_extension("json.tmp");
        let mut file = tokio::fs::File::create(&staging).await?;
        file.write_all(&serde_json::to_vec_pretty(&groups)?).await?;
        file.sync_all().await?;
        tokio::fs::rename(&staging, &self.path).await?;
        self.groups = groups;
        Ok(())
    }
}
