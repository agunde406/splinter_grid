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

pub(super) mod add_batches;
pub(super) mod change_batch_to_submitted;
pub(super) mod get_batch;
pub(super) mod get_batch_status;
pub(super) mod get_failed_batches;
pub(super) mod get_unsubmitted_batches;
pub(super) mod list_batches_by_status;
pub(super) mod update_batch_status;

pub(super) struct BatchTrackingStoreOperations<'a, C> {
    conn: &'a C,
}

impl<'a, C> BatchTrackingStoreOperations<'a, C>
where
    C: diesel::Connection,
{
    pub fn new(conn: &'a C) -> Self {
        BatchTrackingStoreOperations { conn }
    }
}
