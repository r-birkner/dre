use async_trait::async_trait;
use decentralization::SubnetChangeResponse;
use ic_base_types::PrincipalId;
use ic_management_types::{
    requests::{MembershipReplaceRequest, SubnetExtendRequest},
    BlessedVersions, Network, NetworkError, SubnetMembershipChangeProposal,
};
use log::error;
use serde::de::DeserializeOwned;

#[derive(Clone)]
pub struct DashboardBackendClient {
    pub(crate) url: reqwest::Url,
}

impl DashboardBackendClient {
    pub fn new(network: Network, dev: bool) -> DashboardBackendClient {
        Self {
            url: reqwest::Url::parse(if !dev {
                "https://dashboard.internal.dfinity.network/"
            } else {
                "http://localhost:17000/"
            })
            .expect("invalid base url")
            .join("api/proxy/registry/")
            .expect("failed to join url")
            .join(match network {
                Network::Mainnet => "mainnet/",
                Network::Staging => "staging/",
                Network::Url(_) => {
                    unimplemented!("not supported to run dashboard backed operations on arbitrary networks")
                }
            })
            .expect("failed to join url"),
        }
    }

    pub async fn subnet_pending_action(
        &self,
        subnet: PrincipalId,
    ) -> anyhow::Result<Option<SubnetMembershipChangeProposal>> {
        reqwest::Client::new()
            .get(
                self.url
                    .join(&format!("subnet/{subnet}/pending_action"))
                    .map_err(|e| anyhow::anyhow!(e))?,
            )
            .rest_send()
            .await
    }

    pub async fn membership_replace(&self, request: MembershipReplaceRequest) -> anyhow::Result<SubnetChangeResponse> {
        reqwest::Client::new()
            .post(
                self.url
                    .join("subnet/membership/replace")
                    .map_err(|e| anyhow::anyhow!(e))?,
            )
            .json(&request)
            .rest_send()
            .await
    }

    pub async fn subnet_extend(&self, request: SubnetExtendRequest) -> anyhow::Result<SubnetChangeResponse> {
        reqwest::Client::new()
            .post(
                self.url
                    .join("subnet/membership/extend")
                    .map_err(|e| anyhow::anyhow!(e))?,
            )
            .json(&request)
            .rest_send()
            .await
    }

    pub async fn get_nss_blessed_versions(&self) -> anyhow::Result<BlessedVersions> {
        reqwest::Client::new()
            .get(self.url.join("nns/blessed-versions").map_err(|e| anyhow::anyhow!(e))?)
            .rest_send()
            .await
    }
}

#[async_trait]
trait RESTRequestBuilder {
    async fn rest_send<T: DeserializeOwned>(self) -> anyhow::Result<T>;
}

#[async_trait]
impl RESTRequestBuilder for reqwest::RequestBuilder {
    async fn rest_send<T: DeserializeOwned>(self) -> anyhow::Result<T> {
        let response_result = self.send().await?;
        if let Err(e) = response_result.error_for_status_ref() {
            let response = response_result.text().await?;
            match serde_json::from_str(&response) {
                Ok(NetworkError::ExtensionFailed(s)) => {
                    error!("{}", s);
                    Err(anyhow::anyhow!("failed request (error: {})", e))
                }
                _ => Err(anyhow::anyhow!("failed request (error: {}, response: {})", e, response)),
            }
        } else {
            response_result
                .text()
                .await
                .map_err(|e| anyhow::anyhow!(e))
                .and_then(|body| {
                    serde_json::from_str::<T>(&body).map_err(|e| {
                        anyhow::anyhow!(
                            "Error decoding {} from backend output: {}\n{}",
                            std::any::type_name::<T>(),
                            body,
                            e
                        )
                    })
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_backend_client_url() {
        assert_eq!(
            DashboardBackendClient::new(Network::Mainnet, false).url.to_string(),
            "https://dashboard.internal.dfinity.network/api/proxy/registry/mainnet/"
        );
        assert_eq!(
            DashboardBackendClient::new(Network::Staging, false).url.to_string(),
            "https://dashboard.internal.dfinity.network/api/proxy/registry/staging/"
        );
        assert_eq!(
            DashboardBackendClient::new(Network::Mainnet, true).url.to_string(),
            "http://localhost:17000/api/proxy/registry/mainnet/"
        );
        assert_eq!(
            DashboardBackendClient::new(Network::Staging, true).url.to_string(),
            "http://localhost:17000/api/proxy/registry/staging/"
        );
    }
}
