// Copyright 2020 The OctoSQL Authors
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

use std::sync::Arc;

use arrow::compute::kernels::filter;
use arrow::datatypes::{Schema, BooleanType};
use arrow::record_batch::RecordBatch;
use anyhow::Result;

use crate::physical::expression::Expression;
use crate::physical::physical::*;
use crate::logical::logical::NodeMetadata;
use arrow::array::PrimitiveArray;

pub struct Filter {
    logical_metadata: NodeMetadata,
    filter_expr: Arc<dyn Expression>,
    source: Arc<dyn Node>,
}

impl Filter {
    pub fn new(logical_metadata: NodeMetadata, filter_expr: Arc<dyn Expression>, source: Arc<dyn Node>) -> Filter {
        Filter { logical_metadata, filter_expr, source }
    }
}

impl Node for Filter {
    fn logical_metadata(&self) -> NodeMetadata {
        self.logical_metadata.clone()
    }

    fn run(
        &self,
        exec_ctx: &ExecutionContext,
        produce: ProduceFn,
        meta_send: MetaSendFn,
    ) -> Result<()> {
        let source_schema = self.source.logical_metadata().schema;

        self.source.run(
            exec_ctx,
            &mut |ctx, batch| {
                let predicate_column_untyped = self.filter_expr
                    .evaluate(exec_ctx, &batch)?;
                let predicate_column = predicate_column_untyped
                    .as_any()
                    .downcast_ref::<PrimitiveArray<BooleanType>>()
                    .unwrap();
                let new_columns = batch
                    .columns()
                    .into_iter()
                    .map(|array_ref| filter::filter(array_ref.as_ref(), predicate_column).unwrap())
                    .collect();
                let new_batch = RecordBatch::try_new(source_schema.clone(), new_columns).unwrap();
                if new_batch.num_rows() > 0 {
                    produce(ctx, new_batch)?;
                }
                Ok(())
            },
            meta_send,
        )?;
        Ok(())
    }
}
