// Copyright 2022 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub mod models;
mod operations;
pub(crate) mod schema;

use diesel::connection::AnsiTransactionManager;
use diesel::r2d2::{ConnectionManager, Pool};

use super::{
    BatchStatus, BatchStatusName, BatchTrackingStore, BatchTrackingStoreError, InvalidTransaction,
    SubmissionError, TrackingBatch, TrackingBatchList, TrackingTransaction, TransactionReceipt,
    ValidTransaction,
};

use crate::error::ResourceTemporarilyUnavailableError;

use models::{NewBatchStatusModel, NewSubmissionModel, TransactionReceiptModel};
use operations::add_batches::BatchTrackingStoreAddBatchesOperation as _;
use operations::change_batch_to_submitted::BatchTrackingStoreChangeBatchToSubmittedOperation as _;
use operations::clean_stale_records::BatchTrackingCleanStaleRecordsOperation as _;
use operations::get_batch::BatchTrackingStoreGetBatchOperation as _;
use operations::get_batch_status::BatchTrackingStoreGetBatchStatusOperation as _;
use operations::get_failed_batches::BatchTrackingStoreGetFailedBatchesOperation as _;
use operations::get_unsubmitted_batches::BatchTrackingStoreGetUnsubmittedBatchesOperation as _;
use operations::list_batches_by_status::BatchTrackingStoreListBatchesByStatusOperation as _;
use operations::update_batch_status::BatchTrackingStoreUpdateBatchStatusOperation as _;
use operations::BatchTrackingStoreOperations;

/// Manages batches in the database
#[derive(Clone)]
pub struct DieselBatchTrackingStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselBatchTrackingStore<C> {
    /// Creates a new DieselBatchTrackingStore
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool to the database
    #[allow(dead_code)]
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselBatchTrackingStore { connection_pool }
    }
}

#[cfg(feature = "postgres")]
impl BatchTrackingStore for DieselBatchTrackingStore<diesel::pg::PgConnection> {
    fn get_batch_status(
        &self,
        id: &str,
        service_id: &str,
    ) -> Result<Option<BatchStatus>, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .get_batch_status(id, service_id)
    }

    fn update_batch_status(
        &self,
        id: &str,
        service_id: &str,
        status: Option<BatchStatus>,
        transaction_receipts: Vec<TransactionReceipt>,
        submission_error: Option<SubmissionError>,
    ) -> Result<(), BatchTrackingStoreError> {
        let rcpts: Vec<TransactionReceiptModel> = transaction_receipts
            .iter()
            .map(|t| TransactionReceiptModel::from((t, service_id)))
            .collect::<Vec<TransactionReceiptModel>>();

        let stat = status.map(|s| s.to_string());

        let batch_status: Option<&str> = stat.as_deref();

        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .update_batch_status(id, service_id, batch_status, rcpts, submission_error)
    }

    fn add_batches(&self, batches: Vec<TrackingBatch>) -> Result<(), BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .add_batches(batches)
    }

    fn change_batch_to_submitted(
        &self,
        batch_id: &str,
        service_id: &str,
        transaction_receipts: Vec<TransactionReceipt>,
        dlt_status: Option<&str>,
        submission_error: Option<SubmissionError>,
    ) -> Result<(), BatchTrackingStoreError> {
        let mut batch_status = None;

        if let Some(ds) = dlt_status {
            batch_status = Some(NewBatchStatusModel {
                batch_id: batch_id.to_string(),
                service_id: service_id.to_string(),
                dlt_status: ds.to_string(),
            });
        }

        let mut submission = NewSubmissionModel {
            batch_id: batch_id.to_string(),
            service_id: service_id.to_string(),
            error_type: None,
            error_message: None,
        };

        if let Some(s) = submission_error {
            submission = NewSubmissionModel {
                batch_id: batch_id.to_string(),
                service_id: service_id.to_string(),
                error_type: Some(s.error_type().to_string()),
                error_message: Some(s.error_message().to_string()),
            };
        }

        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .change_batch_to_submitted(
            batch_id,
            service_id,
            transaction_receipts
                .iter()
                .map(|r| TransactionReceiptModel::from((r, service_id)))
                .collect(),
            batch_status,
            submission,
        )
    }

    fn get_batch(
        &self,
        id: &str,
        service_id: &str,
    ) -> Result<Option<TrackingBatch>, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .get_batch(id, service_id)
    }

    fn list_batches_by_status(
        &self,
        status: BatchStatus,
    ) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .list_batches_by_status(&status.to_string())
    }

    fn clean_stale_records(&self, submitted_by: i64) -> Result<(), BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .clean_stale_records(submitted_by)
    }

    fn get_unsubmitted_batches(&self) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .get_unsubmitted_batches()
    }

    fn get_failed_batches(&self) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .get_failed_batches()
    }
}

#[cfg(feature = "sqlite")]
impl BatchTrackingStore for DieselBatchTrackingStore<diesel::sqlite::SqliteConnection> {
    fn get_batch_status(
        &self,
        id: &str,
        service_id: &str,
    ) -> Result<Option<BatchStatus>, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .get_batch_status(id, service_id)
    }

    fn update_batch_status(
        &self,
        id: &str,
        service_id: &str,
        status: Option<BatchStatus>,
        transaction_receipts: Vec<TransactionReceipt>,
        submission_error: Option<SubmissionError>,
    ) -> Result<(), BatchTrackingStoreError> {
        let rcpts: Vec<TransactionReceiptModel> = transaction_receipts
            .iter()
            .map(|t| TransactionReceiptModel::from((t, service_id)))
            .collect::<Vec<TransactionReceiptModel>>();

        let stat = status.map(|s| s.to_string());

        let batch_status: Option<&str> = stat.as_deref();

        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .update_batch_status(id, service_id, batch_status, rcpts, submission_error)
    }

    fn add_batches(&self, batches: Vec<TrackingBatch>) -> Result<(), BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .add_batches(batches)
    }

    fn change_batch_to_submitted(
        &self,
        batch_id: &str,
        service_id: &str,
        transaction_receipts: Vec<TransactionReceipt>,
        dlt_status: Option<&str>,
        submission_error: Option<SubmissionError>,
    ) -> Result<(), BatchTrackingStoreError> {
        let mut batch_status = None;

        if let Some(ds) = dlt_status {
            batch_status = Some(NewBatchStatusModel {
                batch_id: batch_id.to_string(),
                service_id: service_id.to_string(),
                dlt_status: ds.to_string(),
            });
        }

        let mut submission = NewSubmissionModel {
            batch_id: batch_id.to_string(),
            service_id: service_id.to_string(),
            error_type: None,
            error_message: None,
        };

        if let Some(s) = submission_error {
            submission = NewSubmissionModel {
                batch_id: batch_id.to_string(),
                service_id: service_id.to_string(),
                error_type: Some(s.error_type().to_string()),
                error_message: Some(s.error_message().to_string()),
            };
        }

        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .change_batch_to_submitted(
            batch_id,
            service_id,
            transaction_receipts
                .iter()
                .map(|r| TransactionReceiptModel::from((r, service_id)))
                .collect(),
            batch_status,
            submission,
        )
    }

    fn get_batch(
        &self,
        id: &str,
        service_id: &str,
    ) -> Result<Option<TrackingBatch>, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .get_batch(id, service_id)
    }

    fn list_batches_by_status(
        &self,
        status: BatchStatus,
    ) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .list_batches_by_status(&status.to_string())
    }

    fn clean_stale_records(&self, submitted_by: i64) -> Result<(), BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .clean_stale_records(submitted_by)
    }

    fn get_unsubmitted_batches(&self) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .get_unsubmitted_batches()
    }

    fn get_failed_batches(&self) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(&*self.connection_pool.get().map_err(|err| {
            BatchTrackingStoreError::ResourceTemporarilyUnavailableError(
                ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
            )
        })?)
        .get_failed_batches()
    }
}

pub struct DieselConnectionBatchTrackingStore<'a, C>
where
    C: diesel::Connection<TransactionManager = AnsiTransactionManager> + 'static,
    C::Backend: diesel::backend::UsesAnsiSavepointSyntax,
{
    connection: &'a C,
}

impl<'a, C> DieselConnectionBatchTrackingStore<'a, C>
where
    C: diesel::Connection<TransactionManager = AnsiTransactionManager> + 'static,
    C::Backend: diesel::backend::UsesAnsiSavepointSyntax,
{
    #[allow(dead_code)]
    pub fn new(connection: &'a C) -> Self {
        DieselConnectionBatchTrackingStore { connection }
    }
}

#[cfg(feature = "postgres")]
impl<'a> BatchTrackingStore for DieselConnectionBatchTrackingStore<'a, diesel::pg::PgConnection> {
    fn get_batch_status(
        &self,
        id: &str,
        service_id: &str,
    ) -> Result<Option<BatchStatus>, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).get_batch_status(id, service_id)
    }

    fn update_batch_status(
        &self,
        id: &str,
        service_id: &str,
        status: Option<BatchStatus>,
        transaction_receipts: Vec<TransactionReceipt>,
        submission_error: Option<SubmissionError>,
    ) -> Result<(), BatchTrackingStoreError> {
        let rcpts: Vec<TransactionReceiptModel> = transaction_receipts
            .iter()
            .map(|t| TransactionReceiptModel::from((t, service_id)))
            .collect::<Vec<TransactionReceiptModel>>();

        let stat = status.map(|s| s.to_string());

        let batch_status: Option<&str> = stat.as_deref();

        BatchTrackingStoreOperations::new(self.connection).update_batch_status(
            id,
            service_id,
            batch_status,
            rcpts,
            submission_error,
        )
    }

    fn add_batches(&self, batches: Vec<TrackingBatch>) -> Result<(), BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).add_batches(batches)
    }

    fn change_batch_to_submitted(
        &self,
        batch_id: &str,
        service_id: &str,
        transaction_receipts: Vec<TransactionReceipt>,
        dlt_status: Option<&str>,
        submission_error: Option<SubmissionError>,
    ) -> Result<(), BatchTrackingStoreError> {
        let mut batch_status = None;

        if let Some(ds) = dlt_status {
            batch_status = Some(NewBatchStatusModel {
                batch_id: batch_id.to_string(),
                service_id: service_id.to_string(),
                dlt_status: ds.to_string(),
            });
        }

        let mut submission = NewSubmissionModel {
            batch_id: batch_id.to_string(),
            service_id: service_id.to_string(),
            error_type: None,
            error_message: None,
        };

        if let Some(s) = submission_error {
            submission = NewSubmissionModel {
                batch_id: batch_id.to_string(),
                service_id: service_id.to_string(),
                error_type: Some(s.error_type().to_string()),
                error_message: Some(s.error_message().to_string()),
            };
        }

        BatchTrackingStoreOperations::new(self.connection).change_batch_to_submitted(
            batch_id,
            service_id,
            transaction_receipts
                .iter()
                .map(|r| TransactionReceiptModel::from((r, service_id)))
                .collect(),
            batch_status,
            submission,
        )
    }

    fn get_batch(
        &self,
        id: &str,
        service_id: &str,
    ) -> Result<Option<TrackingBatch>, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).get_batch(id, service_id)
    }

    fn list_batches_by_status(
        &self,
        status: BatchStatus,
    ) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection)
            .list_batches_by_status(&status.to_string())
    }

    fn clean_stale_records(&self, submitted_by: i64) -> Result<(), BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).clean_stale_records(submitted_by)
    }

    fn get_unsubmitted_batches(&self) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).get_unsubmitted_batches()
    }

    fn get_failed_batches(&self) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).get_failed_batches()
    }
}

#[cfg(feature = "sqlite")]
impl<'a> BatchTrackingStore
    for DieselConnectionBatchTrackingStore<'a, diesel::sqlite::SqliteConnection>
{
    fn get_batch_status(
        &self,
        id: &str,
        service_id: &str,
    ) -> Result<Option<BatchStatus>, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).get_batch_status(id, service_id)
    }

    fn update_batch_status(
        &self,
        id: &str,
        service_id: &str,
        status: Option<BatchStatus>,
        transaction_receipts: Vec<TransactionReceipt>,
        submission_error: Option<SubmissionError>,
    ) -> Result<(), BatchTrackingStoreError> {
        let rcpts: Vec<TransactionReceiptModel> = transaction_receipts
            .iter()
            .map(|t| TransactionReceiptModel::from((t, service_id)))
            .collect::<Vec<TransactionReceiptModel>>();

        let stat = status.map(|s| s.to_string());

        let batch_status: Option<&str> = stat.as_deref();

        BatchTrackingStoreOperations::new(self.connection).update_batch_status(
            id,
            service_id,
            batch_status,
            rcpts,
            submission_error,
        )
    }

    fn add_batches(&self, batches: Vec<TrackingBatch>) -> Result<(), BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).add_batches(batches)
    }

    fn change_batch_to_submitted(
        &self,
        batch_id: &str,
        service_id: &str,
        transaction_receipts: Vec<TransactionReceipt>,
        dlt_status: Option<&str>,
        submission_error: Option<SubmissionError>,
    ) -> Result<(), BatchTrackingStoreError> {
        let mut batch_status = None;

        if let Some(ds) = dlt_status {
            batch_status = Some(NewBatchStatusModel {
                batch_id: batch_id.to_string(),
                service_id: service_id.to_string(),
                dlt_status: ds.to_string(),
            });
        }

        let mut submission = NewSubmissionModel {
            batch_id: batch_id.to_string(),
            service_id: service_id.to_string(),
            error_type: None,
            error_message: None,
        };

        if let Some(s) = submission_error {
            submission = NewSubmissionModel {
                batch_id: batch_id.to_string(),
                service_id: service_id.to_string(),
                error_type: Some(s.error_type().to_string()),
                error_message: Some(s.error_message().to_string()),
            };
        }

        BatchTrackingStoreOperations::new(self.connection).change_batch_to_submitted(
            batch_id,
            service_id,
            transaction_receipts
                .iter()
                .map(|r| TransactionReceiptModel::from((r, service_id)))
                .collect(),
            batch_status,
            submission,
        )
    }

    fn get_batch(
        &self,
        id: &str,
        service_id: &str,
    ) -> Result<Option<TrackingBatch>, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).get_batch(id, service_id)
    }

    fn list_batches_by_status(
        &self,
        status: BatchStatus,
    ) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection)
            .list_batches_by_status(&status.to_string())
    }

    fn clean_stale_records(&self, submitted_by: i64) -> Result<(), BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).clean_stale_records(submitted_by)
    }

    fn get_unsubmitted_batches(&self) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).get_unsubmitted_batches()
    }

    fn get_failed_batches(&self) -> Result<TrackingBatchList, BatchTrackingStoreError> {
        BatchTrackingStoreOperations::new(self.connection).get_failed_batches()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cylinder::{secp256k1::Secp256k1Context, Context, Signer};
    use diesel::r2d2::{ConnectionManager, Pool};
    use diesel::sqlite::SqliteConnection;
    use transact::protocol::{
        batch::{Batch, BatchBuilder},
        transaction::{HashMethod, Transaction, TransactionBuilder},
    };

    use crate::batch_tracking::store::{
        BatchBuilderError, InvalidTransactionBuilder, SubmissionErrorBuilder, TrackingBatchBuilder,
        TransactionReceiptBuilder,
    };
    use crate::hex;
    use crate::migrations::run_sqlite_migrations;

    static FAMILY_NAME: &str = "test_family";
    static FAMILY_VERSION: &str = "0.1";
    static KEY1: &str = "111111111111111111111111111111111111111111111111111111111111111111";
    static KEY2: &str = "222222222222222222222222222222222222222222222222222222222222222222";
    static KEY3: &str = "333333333333333333333333333333333333333333333333333333333333333333";
    static KEY4: &str = "444444444444444444444444444444444444444444444444444444444444444444";
    static KEY5: &str = "555555555555555555555555555555555555555555555555555555555555555555";
    static KEY6: &str = "666666666666666666666666666666666666666666666666666666666666666666";
    static KEY7: &str = "777777777777777777777777777777777777777777777777777777777777777777";
    static NONCE: &str = "f9kdzz";
    static NONCE2: &str = "dzzf9k";
    static BYTES2: [u8; 4] = [0x05, 0x06, 0x07, 0x08];

    #[test]
    fn add_and_fetch_batch() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let add_tracking_batch = get_tracking_batch(batch_1.clone(), false)
            .build()
            .expect("Failed to build batch");

        let id = add_tracking_batch.batch_header();

        store
            .add_batches(vec![add_tracking_batch.clone()])
            .expect("Failed to add batch");

        let batch_result = store
            .get_batch(&id, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_timestamp = batch_result.created_at();

        let expected = get_tracking_batch(batch_1.clone(), false)
            .with_created_at(batch_timestamp)
            .build()
            .expect("Failed to build batch");

        assert_eq!(batch_result, expected);
    }

    #[test]
    fn add_and_fetch_batch_with_dcid() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let dcid = "dcid:data_change".to_string();

        let add_tracking_batch = get_tracking_batch(batch_1.clone(), false)
            .with_data_change_id(dcid.clone())
            .build()
            .expect("Failed to build batch");

        store
            .add_batches(vec![add_tracking_batch.clone()])
            .expect("Failed to add batch");

        let batch_result = store
            .get_batch(&dcid, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_timestamp = batch_result.created_at();
        let expected = get_tracking_batch(batch_1.clone(), false)
            .with_created_at(batch_timestamp)
            .with_data_change_id(dcid)
            .build()
            .expect("Failed to build batch");

        assert_eq!(batch_result, expected);
    }

    #[test]
    fn test_invalid_dcid() {
        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let dcid = "data_change".to_string();

        let add_tracking_batch = TrackingBatchBuilder::default()
            .with_batch(batch_1.clone())
            .with_service_id("TEST".to_string())
            .with_data_change_id(dcid.clone())
            .with_signer_public_key(KEY1.to_string())
            .with_submitted(false)
            .build();

        assert!(add_tracking_batch.is_err());

        let expected = BatchBuilderError::MissingRequiredField(
            "data change IDs must be formatted as 'dcid:<id>'".to_string(),
        );

        assert_eq!(
            add_tracking_batch.unwrap_err().to_string(),
            expected.to_string()
        );
    }

    #[test]
    fn update_batch_status() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let transaction_id = pair.header_signature().to_string();

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let tracking_batch = get_tracking_batch(batch_1.clone(), false)
            .build()
            .expect("Failed to build batch");

        let id = tracking_batch.batch_header();

        store
            .add_batches(vec![tracking_batch.clone()])
            .expect("Failed to add batch");

        store
            .update_batch_status(
                &tracking_batch.batch_header(),
                "TEST",
                Some(BatchStatus::Pending),
                Vec::new(),
                None,
            )
            .expect("Failed to update batch");

        let batch_result_1 = store
            .get_batch(&id, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_result_1_timestamp = batch_result_1.created_at();

        let expected_1 = get_tracking_batch(batch_1.clone(), true)
            .with_created_at(batch_result_1_timestamp)
            .with_batch_status(BatchStatus::Pending)
            .build()
            .expect("Failed to build batch");

        assert_eq!(batch_result_1, expected_1);

        let submission_error = SubmissionErrorBuilder::default()
            .with_error_type("test".to_string())
            .with_error_message("test message".to_string())
            .build()
            .expect("Failed to build error");

        let receipt_1 = TransactionReceiptBuilder::default()
            .with_transaction_id(transaction_id.to_string())
            .with_result_valid(false)
            .with_error_message("test".to_string())
            .with_error_data(BYTES2.to_vec())
            .with_serialized_receipt(
                std::str::from_utf8(&BYTES2)
                    .expect("Failed to build string")
                    .to_string(),
            )
            .build()
            .expect("Failed to build receipt");

        let invalid_transactions = vec![InvalidTransactionBuilder::default()
            .with_transaction_id(transaction_id.to_string())
            .with_error_message("test".to_string())
            .with_error_data(BYTES2.to_vec())
            .build()
            .expect("Failed to build invalid transaction")];

        store
            .update_batch_status(
                &tracking_batch.batch_header(),
                "TEST",
                Some(BatchStatus::Invalid(invalid_transactions.to_vec())),
                vec![receipt_1],
                Some(submission_error.clone()),
            )
            .expect("Failed to update batch");

        let batch_result_2 = store
            .get_batch(&id, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_result_2_timestamp = batch_result_2.created_at();

        let expected_2 = get_tracking_batch(batch_1.clone(), true)
            .with_created_at(batch_result_2_timestamp)
            .with_batch_status(BatchStatus::Invalid(invalid_transactions))
            .with_submission_error(submission_error)
            .build()
            .expect("Failed to build batch");

        assert_eq!(batch_result_2, expected_2);
    }

    #[test]
    fn update_batch_status_dcid() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let transaction_id = pair.header_signature().to_string();

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let dcid = "dcid:data_change".to_string();

        let tracking_batch = get_tracking_batch(batch_1.clone(), false)
            .with_data_change_id(dcid.clone())
            .build()
            .expect("Failed to build batch");

        store
            .add_batches(vec![tracking_batch.clone()])
            .expect("Failed to add batch");

        store
            .update_batch_status(&dcid, "TEST", Some(BatchStatus::Pending), Vec::new(), None)
            .expect("Failed to update batch");

        let batch_result_1 = store
            .get_batch(&dcid, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_result_1_timestamp = batch_result_1.created_at();

        let expected_1 = get_tracking_batch(batch_1.clone(), true)
            .with_created_at(batch_result_1_timestamp)
            .with_batch_status(BatchStatus::Pending)
            .with_data_change_id(dcid.clone())
            .build()
            .expect("Failed to build batch");

        assert_eq!(batch_result_1, expected_1);

        let submission_error = SubmissionErrorBuilder::default()
            .with_error_type("test".to_string())
            .with_error_message("test message".to_string())
            .build()
            .expect("Failed to build error");

        let receipt_1 = TransactionReceiptBuilder::default()
            .with_transaction_id(transaction_id.to_string())
            .with_result_valid(false)
            .with_error_message("test".to_string())
            .with_error_data(BYTES2.to_vec())
            .with_serialized_receipt(
                std::str::from_utf8(&BYTES2)
                    .expect("Failed to build string")
                    .to_string(),
            )
            .build()
            .expect("Failed to build receipt");

        let invalid_transactions = vec![InvalidTransactionBuilder::default()
            .with_transaction_id(transaction_id.to_string())
            .with_error_message("test".to_string())
            .with_error_data(BYTES2.to_vec())
            .build()
            .expect("Failed to build invalid transaction")];

        store
            .update_batch_status(
                &dcid,
                "TEST",
                Some(BatchStatus::Invalid(invalid_transactions.to_vec())),
                vec![receipt_1],
                Some(submission_error.clone()),
            )
            .expect("Failed to update batch");

        let batch_result_2 = store
            .get_batch(&dcid, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_result_2_timestamp = batch_result_2.created_at();

        let expected_2 = get_tracking_batch(batch_1.clone(), true)
            .with_created_at(batch_result_2_timestamp)
            .with_batch_status(BatchStatus::Invalid(invalid_transactions))
            .with_submission_error(submission_error)
            .with_data_change_id(dcid.clone())
            .build()
            .expect("Failed to build batch");

        assert_eq!(batch_result_2, expected_2);
    }

    #[test]
    fn change_batch_to_submitted() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let transaction_header = pair.header_signature().to_string();

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let tracking_batch = get_tracking_batch(batch_1.clone(), false)
            .build()
            .expect("Failed to build batch");

        let id = tracking_batch.batch_header();

        let txn_receipts = vec![TransactionReceiptBuilder::default()
            .with_transaction_id(transaction_header)
            .with_result_valid(true)
            .with_serialized_receipt(
                std::str::from_utf8(&BYTES2)
                    .expect("Failed to build string")
                    .to_string(),
            )
            .build()
            .expect("Failed to build receipt")];

        let submission_error = SubmissionErrorBuilder::default()
            .with_error_type("test".to_string())
            .with_error_message("test message".to_string())
            .build()
            .expect("Failed to build error");

        store
            .add_batches(vec![tracking_batch.clone()])
            .expect("Failed to add batch");

        store
            .change_batch_to_submitted(
                &id,
                "TEST",
                txn_receipts,
                Some("Pending"),
                Some(submission_error),
            )
            .expect("Failed to change batch to submitted");

        let batch = store
            .get_batch(&id, "TEST")
            .expect("Failed to get batch")
            .unwrap();

        assert_eq!(batch.submitted(), true)
    }

    #[test]
    fn change_batch_to_submitted_dcid() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let transaction_header = pair.header_signature().to_string();

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let dcid = "dcid:data_change".to_string();

        let tracking_batch = get_tracking_batch(batch_1.clone(), false)
            .with_data_change_id(dcid.clone())
            .build()
            .expect("Failed to build batch");

        let txn_receipts = vec![TransactionReceiptBuilder::default()
            .with_transaction_id(transaction_header)
            .with_result_valid(true)
            .with_serialized_receipt(
                std::str::from_utf8(&BYTES2)
                    .expect("Failed to build string")
                    .to_string(),
            )
            .build()
            .expect("Failed to build receipt")];

        let submission_error = SubmissionErrorBuilder::default()
            .with_error_type("test".to_string())
            .with_error_message("test message".to_string())
            .build()
            .expect("Failed to build error");

        store
            .add_batches(vec![tracking_batch.clone()])
            .expect("Failed to add batch");

        store
            .change_batch_to_submitted(
                &dcid,
                "TEST",
                txn_receipts,
                Some("Pending"),
                Some(submission_error),
            )
            .expect("Failed to change batch to submitted");

        let batch = store
            .get_batch(&dcid, "TEST")
            .expect("Failed to get batch")
            .unwrap();

        assert_eq!(batch.submitted(), true)
    }

    #[test]
    fn change_batch_to_submitted_no_batch() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let res = store
            .change_batch_to_submitted("id", "TEST", Vec::new(), Some("Pending"), None)
            .unwrap_err();

        assert_eq!(
            res.to_string(),
            BatchTrackingStoreError::NotFoundError("Could not find batch with ID id".to_string())
                .to_string()
        );
    }

    #[test]
    fn get_unsubmitted_batches() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair_1 = get_transact_transaction(&*signer, NONCE);

        let pair_2 = get_transact_transaction(&*signer, NONCE2);

        let batch_1 = get_transact_batch(&*signer, vec![pair_1]);

        let batch_2 = get_transact_batch(&*signer, vec![pair_2]);

        let tracking_batch_1 = get_tracking_batch(batch_1.clone(), false)
            .build()
            .expect("Failed to build batch");

        let id = tracking_batch_1.batch_header();

        let tracking_batch_2 = get_tracking_batch(batch_2.clone(), true)
            .build()
            .expect("Failed to build batch");

        store
            .add_batches(vec![tracking_batch_1.clone(), tracking_batch_2])
            .expect("Failed to add batches");

        let batch_result = store
            .get_batch(&id, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_result_timestamp = batch_result.created_at();

        let expected_batch = get_tracking_batch(batch_1.clone(), false)
            .with_created_at(batch_result_timestamp)
            .build()
            .expect("Failed to build batch");

        let expected = TrackingBatchList {
            batches: vec![expected_batch],
        };

        assert_eq!(
            store
                .get_unsubmitted_batches()
                .expect("Failed to get batch"),
            expected
        );
    }

    #[test]
    fn test_list_pending_batches() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let tracking_batch = get_tracking_batch(batch_1.clone(), false)
            .build()
            .expect("Failed to build batch");

        let id = tracking_batch.batch_header();

        store
            .add_batches(vec![tracking_batch.clone()])
            .expect("Failed to add batch");

        let batch_result = store
            .get_batch(&id, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_result_timestamp = batch_result.created_at();

        store
            .update_batch_status(&id, "TEST", Some(BatchStatus::Pending), Vec::new(), None)
            .expect("Failed to update batch");

        let expected = get_tracking_batch(batch_1.clone(), true)
            .with_created_at(batch_result_timestamp)
            .with_batch_status(BatchStatus::Pending)
            .build()
            .expect("Failed to build batch");

        assert_eq!(
            store
                .list_batches_by_status(BatchStatus::Pending)
                .expect("Failed to get batch"),
            TrackingBatchList {
                batches: vec![expected]
            }
        );

        store
            .update_batch_status(&id, "TEST", Some(BatchStatus::Unknown), Vec::new(), None)
            .expect("Failed to update batch");

        assert_eq!(
            store
                .list_batches_by_status(BatchStatus::Pending)
                .expect("Failed to get batch"),
            TrackingBatchList {
                batches: Vec::new()
            }
        );
    }

    #[test]
    fn test_get_failed_batches() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let transaction_id = pair.header_signature().to_string();

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let tracking_batch = get_tracking_batch(batch_1.clone(), false)
            .build()
            .expect("Failed to build batch");

        let id = tracking_batch.batch_header();

        store
            .add_batches(vec![tracking_batch.clone()])
            .expect("Failed to add batch");

        let batch_result = store
            .get_batch(&id, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_result_timestamp = batch_result.created_at();

        let submission_error = SubmissionErrorBuilder::default()
            .with_error_type("test".to_string())
            .with_error_message("test message".to_string())
            .build()
            .expect("Failed to build error");

        let receipt_1 = TransactionReceiptBuilder::default()
            .with_transaction_id(transaction_id.to_string())
            .with_result_valid(false)
            .with_error_message("test".to_string())
            .with_error_data(BYTES2.to_vec())
            .with_serialized_receipt(
                std::str::from_utf8(&BYTES2)
                    .expect("Failed to build string")
                    .to_string(),
            )
            .build()
            .expect("Failed to build receipt");

        let invalid_transactions = vec![InvalidTransactionBuilder::default()
            .with_transaction_id(transaction_id.to_string())
            .with_error_message("test".to_string())
            .with_error_data(BYTES2.to_vec())
            .build()
            .expect("Failed to build transaction")];

        store
            .update_batch_status(
                &id,
                "TEST",
                Some(BatchStatus::Invalid(invalid_transactions.to_vec())),
                vec![receipt_1],
                Some(submission_error.clone()),
            )
            .expect("Failed to update batch");

        let expected = get_tracking_batch(batch_1.clone(), true)
            .with_created_at(batch_result_timestamp)
            .with_batch_status(BatchStatus::Invalid(invalid_transactions))
            .with_submission_error(submission_error)
            .build()
            .expect("Failed to build batch");

        assert_eq!(
            store.get_failed_batches().expect("Failed to get batch"),
            TrackingBatchList {
                batches: vec![expected]
            }
        );

        store
            .update_batch_status(&id, "TEST", Some(BatchStatus::Pending), Vec::new(), None)
            .expect("Failed to update batch");

        assert_eq!(
            store.get_failed_batches().expect("Failed to get batch"),
            TrackingBatchList {
                batches: Vec::new()
            }
        );
    }

    #[test]
    fn test_clean_stale_records() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselBatchTrackingStore::new(pool);

        let signer = new_signer();

        let pair = get_transact_transaction(&*signer, NONCE);

        let batch_1 = get_transact_batch(&*signer, vec![pair]);

        let tracking_batch = get_tracking_batch(batch_1.clone(), false)
            .build()
            .expect("Failed to build batch");

        let id = tracking_batch.batch_header();

        store
            .add_batches(vec![tracking_batch.clone()])
            .expect("Failed to add batch");

        let batch_result = store
            .get_batch(&id, "TEST")
            .expect("Failed to get batch")
            .unwrap();
        let batch_timestamp = batch_result.created_at();

        store
            .clean_stale_records(batch_timestamp + 1)
            .expect("Failed to clean records");

        assert_eq!(
            store.get_batch(&id, "TEST").expect("Failed to get batch"),
            None
        )
    }

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection ensures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }

    fn new_signer() -> Box<dyn Signer> {
        let context = Secp256k1Context::new();
        let key = context.new_random_private_key();
        context.new_signer(key)
    }

    fn get_transact_transaction(signer: &dyn Signer, nonce: &str) -> Transaction {
        TransactionBuilder::new()
            .with_batcher_public_key(hex::parse_hex(KEY1).unwrap())
            .with_dependencies(vec![KEY2.to_string(), KEY3.to_string()])
            .with_family_name(FAMILY_NAME.to_string())
            .with_family_version(FAMILY_VERSION.to_string())
            .with_inputs(vec![
                hex::parse_hex(KEY4).unwrap(),
                hex::parse_hex(&KEY5[0..4]).unwrap(),
            ])
            .with_nonce(nonce.to_string().into_bytes())
            .with_outputs(vec![
                hex::parse_hex(KEY6).unwrap(),
                hex::parse_hex(&KEY7[0..4]).unwrap(),
            ])
            .with_payload_hash_method(HashMethod::Sha512)
            .with_payload(BYTES2.to_vec())
            .build(&*signer)
            .expect("Failed to build transaction")
    }

    fn get_transact_batch(signer: &dyn Signer, transactions: Vec<Transaction>) -> Batch {
        BatchBuilder::new()
            .with_transactions(transactions)
            .build(&*signer)
            .expect("Failed to build transact batch")
    }

    fn get_tracking_batch(batch: Batch, submitted: bool) -> TrackingBatchBuilder {
        TrackingBatchBuilder::default()
            .with_batch(batch)
            .with_service_id("TEST".to_string())
            .with_signer_public_key(KEY1.to_string())
            .with_submitted(submitted)
    }
}
