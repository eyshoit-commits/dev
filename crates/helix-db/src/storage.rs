use std::{path::PathBuf, sync::Arc};

use heed::{types::*, Database, Env, EnvOpenOptions};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    config::HelixConfig,
    document::DocumentRecord,
    error::{HelixError, HelixResult},
    graph::{EdgeRecord, NodeRecord},
    vector::VectorRecord,
};

const NODE_DB: &str = "nodes";
const EDGE_DB: &str = "edges";
const DOCUMENT_DB: &str = "documents";
const VECTOR_DB: &str = "vectors";

#[derive(Clone)]
pub struct StorageEngine {
    env: Arc<Env>,
    node_db: Database<Str, SerdeBincode<NodeRecord>>,
    edge_db: Database<Str, SerdeBincode<EdgeRecord>>,
    document_db: Database<Str, SerdeBincode<DocumentRecord>>,
    vector_db: Database<Str, SerdeBincode<VectorRecord>>,
}

impl StorageEngine {
    pub fn new(config: &HelixConfig) -> HelixResult<Self> {
        config.ensure_dirs()?;
        let path = config.data_dir.join("lmdb");
        std::fs::create_dir_all(&path).map_err(|err| HelixError::Storage(err.to_string()))?;
        let env = EnvOpenOptions::new()
            .max_dbs(32)
            .map_size(1024 * 1024 * 1024)
            .open(path.as_path())
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        let env = Arc::new(env);
        let node_db = open_db(&env, NODE_DB)?;
        let edge_db = open_db(&env, EDGE_DB)?;
        let document_db = open_db(&env, DOCUMENT_DB)?;
        let vector_db = open_db(&env, VECTOR_DB)?;
        Ok(Self {
            env,
            node_db,
            edge_db,
            document_db,
            vector_db,
        })
    }

    pub fn put_node(&self, node: &NodeRecord) -> HelixResult<()> {
        self.put(&self.node_db, &node.id, node)
    }

    pub fn get_node(&self, id: &str) -> HelixResult<Option<NodeRecord>> {
        self.get(&self.node_db, id)
    }

    pub fn put_edge(&self, edge: &EdgeRecord) -> HelixResult<()> {
        self.put(&self.edge_db, &edge.id, edge)
    }

    pub fn get_edges_by_source(&self, source: &str) -> HelixResult<Vec<EdgeRecord>> {
        let txn = self
            .env
            .read_txn()
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        let mut result = Vec::new();
        for item in self
            .edge_db
            .iter(&txn)
            .map_err(|err| HelixError::Storage(err.to_string()))?
        {
            let (_, edge) = item.map_err(|err| HelixError::Storage(err.to_string()))?;
            if edge.source == source {
                result.push(edge);
            }
        }
        Ok(result)
    }

    pub fn put_document_for_node(
        &self,
        node_id: &str,
        document: &DocumentRecord,
    ) -> HelixResult<()> {
        let key = format!("{node_id}:{}", document.id);
        self.put(&self.document_db, &key, document)
    }

    pub fn insert_document(&self, document: &DocumentRecord) -> HelixResult<()> {
        self.put(&self.document_db, &document.id, document)
    }

    pub fn iter_documents(&self) -> HelixResult<Vec<DocumentRecord>> {
        self.collect_all(&self.document_db)
    }

    pub fn put_vector(&self, vector: &VectorRecord) -> HelixResult<()> {
        self.put(&self.vector_db, &vector.id, vector)
    }

    pub fn iter_vectors(&self) -> HelixResult<Vec<VectorRecord>> {
        self.collect_all(&self.vector_db)
    }

    fn put<T: Serialize>(
        &self,
        db: &Database<Str, SerdeBincode<T>>,
        key: &str,
        value: &T,
    ) -> HelixResult<()> {
        let mut txn = self
            .env
            .write_txn()
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        db.put(&mut txn, key, value)
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        txn.commit()
            .map_err(|err| HelixError::Storage(err.to_string()))
    }

    fn get<T: DeserializeOwned>(
        &self,
        db: &Database<Str, SerdeBincode<T>>,
        key: &str,
    ) -> HelixResult<Option<T>> {
        let txn = self
            .env
            .read_txn()
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        db.get(&txn, key)
            .map_err(|err| HelixError::Storage(err.to_string()))
    }

    fn collect_all<T: DeserializeOwned + Clone>(
        &self,
        db: &Database<Str, SerdeBincode<T>>,
    ) -> HelixResult<Vec<T>> {
        let txn = self
            .env
            .read_txn()
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        let mut result = Vec::new();
        for item in db
            .iter(&txn)
            .map_err(|err| HelixError::Storage(err.to_string()))?
        {
            let (_, value) = item.map_err(|err| HelixError::Storage(err.to_string()))?;
            result.push(value);
        }
        Ok(result)
    }

    pub fn path(&self) -> HelixResult<PathBuf> {
        Ok(self
            .env
            .path()
            .map_err(|err| HelixError::Storage(err.to_string()))?
            .to_path_buf())
    }
}

fn open_db<T>(env: &Arc<Env>, name: &str) -> HelixResult<Database<Str, SerdeBincode<T>>> {
    env.create_database(Some(name))
        .map_err(|err| HelixError::Storage(err.to_string()))
}
