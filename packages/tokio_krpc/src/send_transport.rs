use crate::{
    active_transactions::ActiveTransactions,
    errors::{
        ErrorKind,
        Result,
    },
    messages::{
        PortType,
        Request,
        TransactionId,
    },
    response_future::ResponseFuture,
    responses::{
        FindNodeResponse,
        GetPeersResponse,
        NodeIDResponse,
    },
};
use byteorder::NetworkEndian;
use bytes::ByteOrder;
use failure::ResultExt;
use futures::lock::Mutex;
use krpc_encoding::{
    self as proto,
    Envelope,
    NodeID,
    Query,
};
use rand;
use std::{
    self,
    net::SocketAddr,
};
use tokio::net::udp::split::UdpSocketSendHalf;

pub struct SendTransport {
    socket: Mutex<UdpSocketSendHalf>,
    transactions: ActiveTransactions,
}

// TODO: Queries Are Built in a Wierd Way

impl SendTransport {
    pub fn new(socket: UdpSocketSendHalf, transactions: ActiveTransactions) -> SendTransport {
        SendTransport {
            socket: Mutex::new(socket),
            transactions,
        }
    }

    pub async fn request(
        &self,
        address: SocketAddr,
        transaction_id: TransactionId,
        request: Request,
    ) -> Result<proto::Response> {
        self.send_request(address, transaction_id, request).await?;

        Ok(ResponseFuture::wait_for_tx(transaction_id, self.transactions.clone()).await?)
    }

    /// Adds `transaction_id` to the request and sends it.
    async fn send_request(
        &self,
        address: SocketAddr,
        transaction_id: TransactionId,
        mut request: Request,
    ) -> Result<()> {
        let mut buf = [0u8; 4];
        NetworkEndian::write_u32(&mut buf, transaction_id);
        request.transaction_id.extend_from_slice(&buf);

        Ok(self.send(address, request.into()).await?)
    }

    pub async fn send(&self, address: SocketAddr, message: Envelope) -> Result<()> {
        let encoded = message
            .encode()
            .map_err(|cause| ErrorKind::SendEncodingError { cause })?;

        let mut socket = self.socket.lock().await;

        socket
            .send_to(&encoded, &address)
            .await
            .with_context(|_| ErrorKind::SendError { to: address })?;

        Ok(())
    }

    fn get_transaction_id() -> TransactionId {
        rand::random::<TransactionId>()
    }

    fn build_request(&self, query: Query) -> Request {
        Request {
            transaction_id: Vec::new(),
            version: None,
            query,
            read_only: false,
        }
    }

    pub async fn ping(&self, id: NodeID, address: SocketAddr) -> Result<NodeID> {
        let response = self
            .request(
                address,
                Self::get_transaction_id(),
                self.build_request(Query::Ping { id }),
            )
            .await?;

        Ok(NodeIDResponse::from_response(response)?)
    }

    pub async fn find_node(
        &self,
        id: NodeID,
        address: SocketAddr,
        target: NodeID,
    ) -> Result<FindNodeResponse> {
        let response = self
            .request(
                address,
                Self::get_transaction_id(),
                self.build_request(Query::FindNode { id, target }),
            )
            .await?;

        Ok(FindNodeResponse::from_response(response)?)
    }

    pub async fn get_peers(
        &self,
        id: NodeID,
        address: SocketAddr,
        info_hash: NodeID,
    ) -> Result<GetPeersResponse> {
        let response = self
            .request(
                address,
                Self::get_transaction_id(),
                self.build_request(Query::GetPeers { id, info_hash }),
            )
            .await?;

        Ok(GetPeersResponse::from_response(response)?)
    }

    pub async fn announce_peer(
        &self,
        id: NodeID,
        token: Vec<u8>,
        address: SocketAddr,
        info_hash: NodeID,
        port_type: PortType,
    ) -> Result<NodeID> {
        let (port, implied_port) = match port_type {
            PortType::Implied => (None, true),
            PortType::Port(port) => (Some(port), false),
        };

        let response = self
            .request(
                address,
                Self::get_transaction_id(),
                self.build_request(Query::AnnouncePeer {
                    id,
                    token,
                    info_hash,
                    port,
                    implied_port,
                }),
            )
            .await?;

        Ok(NodeIDResponse::from_response(response)?)
    }
}
