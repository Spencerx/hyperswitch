use std::collections::HashSet;

use async_bb8_diesel::AsyncRunQueryDsl;
use diesel::{
    associations::HasTable,
    query_dsl::methods::{DistinctDsl, FilterDsl, SelectDsl},
    BoolExpressionMethods, ExpressionMethods,
};
use error_stack::{report, ResultExt};

use super::generics;
use crate::{
    enums,
    errors::DatabaseError,
    payout_attempt::{
        PayoutAttempt, PayoutAttemptNew, PayoutAttemptUpdate, PayoutAttemptUpdateInternal,
    },
    schema::{payout_attempt::dsl, payouts as payout_dsl},
    Payouts, PgPooledConn, StorageResult,
};

impl PayoutAttemptNew {
    pub async fn insert(self, conn: &PgPooledConn) -> StorageResult<PayoutAttempt> {
        generics::generic_insert(conn, self).await
    }
}

impl PayoutAttempt {
    pub async fn update_with_attempt_id(
        self,
        conn: &PgPooledConn,
        payout_attempt_update: PayoutAttemptUpdate,
    ) -> StorageResult<Self> {
        match generics::generic_update_with_unique_predicate_get_result::<
            <Self as HasTable>::Table,
            _,
            _,
            _,
        >(
            conn,
            dsl::payout_attempt_id
                .eq(self.payout_attempt_id.to_owned())
                .and(dsl::merchant_id.eq(self.merchant_id.to_owned())),
            PayoutAttemptUpdateInternal::from(payout_attempt_update),
        )
        .await
        {
            Err(error) => match error.current_context() {
                DatabaseError::NoFieldsToUpdate => Ok(self),
                _ => Err(error),
            },
            result => result,
        }
    }

    pub async fn find_by_merchant_id_payout_id(
        conn: &PgPooledConn,
        merchant_id: &common_utils::id_type::MerchantId,
        payout_id: &common_utils::id_type::PayoutId,
    ) -> StorageResult<Self> {
        generics::generic_find_one::<<Self as HasTable>::Table, _, _>(
            conn,
            dsl::merchant_id
                .eq(merchant_id.to_owned())
                .and(dsl::payout_id.eq(payout_id.to_owned())),
        )
        .await
    }

    pub async fn find_by_merchant_id_payout_attempt_id(
        conn: &PgPooledConn,
        merchant_id: &common_utils::id_type::MerchantId,
        payout_attempt_id: &str,
    ) -> StorageResult<Self> {
        generics::generic_find_one::<<Self as HasTable>::Table, _, _>(
            conn,
            dsl::merchant_id
                .eq(merchant_id.to_owned())
                .and(dsl::payout_attempt_id.eq(payout_attempt_id.to_owned())),
        )
        .await
    }

    pub async fn find_by_merchant_id_connector_payout_id(
        conn: &PgPooledConn,
        merchant_id: &common_utils::id_type::MerchantId,
        connector_payout_id: &str,
    ) -> StorageResult<Self> {
        generics::generic_find_one::<<Self as HasTable>::Table, _, _>(
            conn,
            dsl::merchant_id
                .eq(merchant_id.to_owned())
                .and(dsl::connector_payout_id.eq(connector_payout_id.to_owned())),
        )
        .await
    }

    pub async fn find_by_merchant_id_merchant_order_reference_id(
        conn: &PgPooledConn,
        merchant_id_input: &common_utils::id_type::MerchantId,
        merchant_order_reference_id_input: &str,
    ) -> StorageResult<Self> {
        generics::generic_find_one::<<Self as HasTable>::Table, _, _>(
            conn,
            dsl::merchant_id.eq(merchant_id_input.to_owned()).and(
                dsl::merchant_order_reference_id.eq(merchant_order_reference_id_input.to_owned()),
            ),
        )
        .await
    }

    pub async fn update_by_merchant_id_payout_id(
        conn: &PgPooledConn,
        merchant_id: &common_utils::id_type::MerchantId,
        payout_id: &common_utils::id_type::PayoutId,
        payout: PayoutAttemptUpdate,
    ) -> StorageResult<Self> {
        generics::generic_update_with_results::<<Self as HasTable>::Table, _, _, _>(
            conn,
            dsl::merchant_id
                .eq(merchant_id.to_owned())
                .and(dsl::payout_id.eq(payout_id.to_owned())),
            PayoutAttemptUpdateInternal::from(payout),
        )
        .await?
        .first()
        .cloned()
        .ok_or_else(|| {
            report!(DatabaseError::NotFound).attach_printable("Error while updating payout")
        })
    }

    pub async fn update_by_merchant_id_payout_attempt_id(
        conn: &PgPooledConn,
        merchant_id: &common_utils::id_type::MerchantId,
        payout_attempt_id: &str,
        payout: PayoutAttemptUpdate,
    ) -> StorageResult<Self> {
        generics::generic_update_with_results::<<Self as HasTable>::Table, _, _, _>(
            conn,
            dsl::merchant_id
                .eq(merchant_id.to_owned())
                .and(dsl::payout_attempt_id.eq(payout_attempt_id.to_owned())),
            PayoutAttemptUpdateInternal::from(payout),
        )
        .await?
        .first()
        .cloned()
        .ok_or_else(|| {
            report!(DatabaseError::NotFound).attach_printable("Error while updating payout")
        })
    }

    pub async fn get_filters_for_payouts(
        conn: &PgPooledConn,
        payouts: &[Payouts],
        merchant_id: &common_utils::id_type::MerchantId,
    ) -> StorageResult<(
        Vec<String>,
        Vec<enums::Currency>,
        Vec<enums::PayoutStatus>,
        Vec<enums::PayoutType>,
    )> {
        let active_attempt_ids = payouts
            .iter()
            .map(|payout| {
                format!(
                    "{}_{}",
                    payout.payout_id.get_string_repr(),
                    payout.attempt_count.clone()
                )
            })
            .collect::<Vec<String>>();

        let active_payout_ids = payouts
            .iter()
            .map(|payout| payout.payout_id.to_owned())
            .collect::<Vec<common_utils::id_type::PayoutId>>();

        let filter = <Self as HasTable>::table()
            .filter(dsl::merchant_id.eq(merchant_id.to_owned()))
            .filter(dsl::payout_attempt_id.eq_any(active_attempt_ids));

        let payouts_filter = <Payouts as HasTable>::table()
            .filter(payout_dsl::merchant_id.eq(merchant_id.to_owned()))
            .filter(payout_dsl::payout_id.eq_any(active_payout_ids));

        let payout_status: Vec<enums::PayoutStatus> = payouts
            .iter()
            .map(|payout| payout.status)
            .collect::<HashSet<enums::PayoutStatus>>()
            .into_iter()
            .collect();

        let filter_connector = filter
            .clone()
            .select(dsl::connector)
            .distinct()
            .get_results_async::<Option<String>>(conn)
            .await
            .change_context(DatabaseError::Others)
            .attach_printable("Error filtering records by connector")?
            .into_iter()
            .flatten()
            .collect::<Vec<String>>();

        let filter_currency = payouts_filter
            .clone()
            .select(payout_dsl::destination_currency)
            .distinct()
            .get_results_async::<enums::Currency>(conn)
            .await
            .change_context(DatabaseError::Others)
            .attach_printable("Error filtering records by currency")?
            .into_iter()
            .collect::<Vec<enums::Currency>>();

        let filter_payout_method = payouts_filter
            .clone()
            .select(payout_dsl::payout_type)
            .distinct()
            .get_results_async::<Option<enums::PayoutType>>(conn)
            .await
            .change_context(DatabaseError::Others)
            .attach_printable("Error filtering records by payout type")?
            .into_iter()
            .flatten()
            .collect::<Vec<enums::PayoutType>>();

        Ok((
            filter_connector,
            filter_currency,
            payout_status,
            filter_payout_method,
        ))
    }
}
