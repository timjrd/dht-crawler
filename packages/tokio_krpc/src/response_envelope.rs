use krpc_encoding as proto;

pub struct ResponseEnvelope {
    pub transaction_id: Vec<u8>,
    pub response: ResponseType,
}

pub enum ResponseType {
    Error { error: proto::KRPCError },
    Response { response: proto::Response },
}
