#![allow(unused, clippy::expect_used)]

use std::{collections::HashMap, str::FromStr};

use api_models::{
    admin as admin_api, enums as api_enums, payment_methods::RequestPaymentMethodTypes,
};
use common_utils::types::MinorUnit;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use euclid::{
    dirval,
    dssa::graph::{self, CgraphExt},
    frontend::dir,
    types::{NumValue, NumValueRefinement},
};
use hyperswitch_constraint_graph::{CycleCheck, Memoization};
use kgraph_utils::{error::KgraphError, transformers::IntoDirValue, types::CountryCurrencyFilter};

#[cfg(feature = "v1")]
fn build_test_data(
    total_enabled: usize,
    total_pm_types: usize,
) -> hyperswitch_constraint_graph::ConstraintGraph<dir::DirValue> {
    use api_models::{admin::*, payment_methods::*};

    let mut pms_enabled: Vec<PaymentMethodsEnabled> = Vec::new();

    for _ in (0..total_enabled) {
        let mut pm_types: Vec<RequestPaymentMethodTypes> = Vec::new();
        for _ in (0..total_pm_types) {
            pm_types.push(RequestPaymentMethodTypes {
                payment_method_type: api_enums::PaymentMethodType::Credit,
                payment_experience: None,
                card_networks: Some(vec![
                    api_enums::CardNetwork::Visa,
                    api_enums::CardNetwork::Mastercard,
                ]),
                accepted_currencies: Some(AcceptedCurrencies::EnableOnly(vec![
                    api_enums::Currency::USD,
                    api_enums::Currency::INR,
                ])),
                accepted_countries: None,
                minimum_amount: Some(MinorUnit::new(10)),
                maximum_amount: Some(MinorUnit::new(1000)),
                recurring_enabled: Some(true),
                installment_payment_enabled: Some(true),
            });
        }

        pms_enabled.push(PaymentMethodsEnabled {
            payment_method: api_enums::PaymentMethod::Card,
            payment_method_types: Some(pm_types),
        });
    }

    let profile_id = common_utils::generate_profile_id_of_default_length();

    // #[cfg(feature = "v2")]
    // let stripe_account = MerchantConnectorResponse {
    //     connector_type: api_enums::ConnectorType::FizOperations,
    //     connector_name: "stripe".to_string(),
    //     id: common_utils::generate_merchant_connector_account_id_of_default_length(),
    //     connector_account_details: masking::Secret::new(serde_json::json!({})),
    //     disabled: None,
    //     metadata: None,
    //     payment_methods_enabled: Some(pms_enabled),
    //     connector_label: Some("something".to_string()),
    //     frm_configs: None,
    //     connector_webhook_details: None,
    //     profile_id,
    //     applepay_verified_domains: None,
    //     pm_auth_config: None,
    //     status: api_enums::ConnectorStatus::Inactive,
    //     additional_merchant_data: None,
    //     connector_wallets_details: None,
    // };

    #[cfg(feature = "v1")]
    let stripe_account = MerchantConnectorResponse {
        connector_type: api_enums::ConnectorType::FizOperations,
        connector_name: "stripe".to_string(),
        merchant_connector_id:
            common_utils::generate_merchant_connector_account_id_of_default_length(),
        connector_account_details: masking::Secret::new(serde_json::json!({})),
        test_mode: None,
        disabled: None,
        metadata: None,
        payment_methods_enabled: Some(pms_enabled),
        business_country: Some(api_enums::CountryAlpha2::US),
        business_label: Some("hello".to_string()),
        connector_label: Some("something".to_string()),
        business_sub_label: Some("something".to_string()),
        frm_configs: None,
        connector_webhook_details: None,
        profile_id,
        applepay_verified_domains: None,
        pm_auth_config: None,
        status: api_enums::ConnectorStatus::Inactive,
        additional_merchant_data: None,
        connector_wallets_details: None,
    };
    let config = CountryCurrencyFilter {
        connector_configs: HashMap::new(),
        default_configs: None,
    };

    #[cfg(feature = "v1")]
    kgraph_utils::mca::make_mca_graph(vec![stripe_account], &config)
        .expect("Failed graph construction")
}

#[cfg(feature = "v1")]
fn evaluation(c: &mut Criterion) {
    let small_graph = build_test_data(3, 8);
    let big_graph = build_test_data(20, 20);

    c.bench_function("MCA Small Graph Evaluation", |b| {
        b.iter(|| {
            small_graph.key_value_analysis(
                dirval!(Connector = Stripe),
                &graph::AnalysisContext::from_dir_values([
                    dirval!(Connector = Stripe),
                    dirval!(PaymentMethod = Card),
                    dirval!(CardType = Credit),
                    dirval!(CardNetwork = Visa),
                    dirval!(PaymentCurrency = BWP),
                    dirval!(PaymentAmount = 100),
                ]),
                &mut Memoization::new(),
                &mut CycleCheck::new(),
                None,
            );
        });
    });

    c.bench_function("MCA Big Graph Evaluation", |b| {
        b.iter(|| {
            big_graph.key_value_analysis(
                dirval!(Connector = Stripe),
                &graph::AnalysisContext::from_dir_values([
                    dirval!(Connector = Stripe),
                    dirval!(PaymentMethod = Card),
                    dirval!(CardType = Credit),
                    dirval!(CardNetwork = Visa),
                    dirval!(PaymentCurrency = BWP),
                    dirval!(PaymentAmount = 100),
                ]),
                &mut Memoization::new(),
                &mut CycleCheck::new(),
                None,
            );
        });
    });
}

#[cfg(feature = "v1")]
criterion_group!(benches, evaluation);
#[cfg(feature = "v1")]
criterion_main!(benches);

#[cfg(feature = "v2")]
fn main() {}
