use r2d2::{PooledConnection, Pool};
use r2d2_sqlite::{SqliteConnectionManager};
use rusqlite::{params};
use std::ops::Deref;
use super::cli_types::{Node, Subnet};
use super::ops::remove_single_node;
use std::sync::Arc;
use super::cli_types::OperatorConfig;
pub struct ReplacementStateWorker<'a> {
    db: Arc<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>,
    not_added: Vec<(String, String, String)>,
    cfg: &'a OperatorConfig
}
struct ReplacementState{
    waiting: String,
    to_remove: String,
    subnet: String,
}
impl ReplacementStateWorker<'_> {
    pub fn new(db: Arc<Pool<SqliteConnectionManager>>, cfg: &'static OperatorConfig) -> Self {
        db.get().expect("Unable to get pool connection").execute(
            "CREATE TABLE IF NOT EXISTS replacement_queue (waiting TEXT removed TEXT, subnet TEXT)", params![]
        )
        .expect("Unable to create needed database table");
        return ReplacementStateWorker{
            db,
            not_added: Vec::new(),
            cfg
        }
    }
    pub fn add_waited_replacement(&mut self, proposal: String, to_remove: String, subnet: String) {
        self.not_added.push((proposal, to_remove, subnet));
    }

    pub fn update_proposals(self) -> Result<(), anyhow::Error> {
        let conn = self.db.get().expect("Unable to get pool connection");
        let mut insert_stmt = conn.prepare("INSERT INTO replacement_queue VALUES (?, ?. ?").expect("Incorrect SQL statement for updating table");
        for (proposal, to_remove, subnet) in self.not_added.iter() {
            insert_stmt.execute(params![proposal, to_remove, subnet]).expect(&format!("Unable to insert proposal {}", proposal));
        }
        let mut query_stmt = conn.prepare("SELECT * FROM replacement_queue").expect("Querying database file failed");
        let mut waiting = query_stmt.query_map(
            params![], |row| {
                Ok(ReplacementState{
                waiting: row.get(0)?,
                to_remove: row.get(1)?,
                subnet: row.get(2)?
                })
            }
        )?;
        for proposal in waiting {
            //TODO - actually support multithreading here and deal with unwraps.
            let unwrapped = proposal.unwrap();
            let proposal_status = futures::executor::block_on(self.get_proposal_status(unwrapped.waiting));
            if proposal_status {
                remove_single_node(Subnet{id: unwrapped.subnet }, Node{ id: unwrapped.to_remove} , self.cfg);
            } else {};
        }
        Ok(())
    }

    async fn get_proposal_status(&self, proposal_id: String) -> bool {
        /// Didn't quite get there in implementation
        /// The purpose of this function is to query the NNS to see if the proposal has been completed or not
        return true
    }
} 