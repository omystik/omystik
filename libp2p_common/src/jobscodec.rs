use anyhow::Result;
use async_trait::async_trait;
use libp2p::{
    futures::{
        AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt
    },
    request_response::Codec,
    StreamProtocol
};

// Jobs section

#[derive(Debug, Clone)]
pub struct JobsReq(pub Vec<u8>);
#[derive(Debug, Clone)]
pub struct JobsResp(pub Vec<u8>);

#[derive(Clone, Copy, Default)]
pub struct JobsCodec;

#[async_trait]
impl Codec for JobsCodec {
    type Protocol = StreamProtocol;
    type Request = JobsReq;
    type Response = JobsResp;

    async fn read_request<T: AsyncRead + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T
    ) -> std::io::Result<Self::Request> {
        let mut buf = Vec::new();
        AsyncReadExt::read_to_end(io, &mut buf).await?;
        Ok(JobsReq(buf))
    }

    async fn read_response<T: AsyncRead + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T
    ) -> std::io::Result<Self::Response> {
        let mut buf = Vec::new();
        AsyncReadExt::read_to_end(io, &mut buf).await?;
        Ok(JobsResp(buf))
    }

    async fn write_request<T: AsyncWrite + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T, JobsReq(data): JobsReq
    ) -> std::io::Result<()> {
        AsyncWriteExt::write_all(io, &data).await?;
        AsyncWriteExt::close(io).await?;
        Ok(())
    }

    async fn write_response<T: AsyncWrite + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T, JobsResp(data): JobsResp
    ) -> std::io::Result<()> {
        AsyncWriteExt::write_all(io, &data).await?;
        AsyncWriteExt::close(io).await?;
        Ok(())
    }
}

// Blobs section
pub async fn write_frame_blob<W: AsyncWriteExt + Unpin>(w: &mut W, bytes: &[u8]) -> Result<()> {
    let len = bytes.len() as u32;
    w.write_all(&len.to_be_bytes()).await?;
    w.write_all(bytes).await?;
    Ok(())
}

pub async fn read_frame_blob<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<Vec<u8>> {
    let mut lenb = [0u8; 4];
    r.read_exact(&mut lenb).await?;
    let len = u32::from_be_bytes(lenb) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(buf)
}