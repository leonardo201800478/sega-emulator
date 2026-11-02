//! Resumable Z80 bus-cycle handshake primitives.
//!
//! This module is deliberately narrower than an instruction microsequencer:
//! it owns one outstanding bus transaction, stable request identity, WAIT
//! extension, and completion validation. Opcode decoding and architectural
//! instruction semantics remain in the existing executor.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80BusOperation {
    OpcodeFetch,
    MemoryRead,
    MemoryWrite { value: u8 },
    PortRead,
    PortWrite { value: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80BusRequest {
    pub id: u64,
    pub operation: Z80BusOperation,
    pub address: u16,
    pub base_tstates: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80BusResponse {
    pub request_id: u64,
    pub data: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80BusHandshakeError {
    RequestAlreadyPending,
    NoRequestPending,
    WrongRequestId { expected: u64, received: u64 },
    MissingReadData,
    UnexpectedWriteData,
    RequestIdExhausted,
    WaitCounterExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingCycle {
    request: Z80BusRequest,
    wait_tstates: u32,
}

/// One outstanding transaction. The machine scheduler remains the sole owner
/// of emulated time; callers insert WAIT states as the scheduled bus cycle runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Z80BusHandshake {
    next_request_id: u64,
    pending: Option<PendingCycle>,
}

impl Z80BusHandshake {
    /// Start a transaction, refusing to overwrite an unfinished request.
    pub fn begin(
        &mut self,
        operation: Z80BusOperation,
        address: u16,
        base_tstates: u8,
    ) -> Result<Z80BusRequest, Z80BusHandshakeError> {
        if self.pending.is_some() {
            return Err(Z80BusHandshakeError::RequestAlreadyPending);
        }
        let id = self.next_request_id;
        self.next_request_id = id
            .checked_add(1)
            .ok_or(Z80BusHandshakeError::RequestIdExhausted)?;
        let request = Z80BusRequest {
            id,
            operation,
            address,
            base_tstates,
        };
        self.pending = Some(PendingCycle {
            request,
            wait_tstates: 0,
        });
        Ok(request)
    }

    /// Inspect the outstanding request without changing its identity.
    pub fn pending_request(&self) -> Option<Z80BusRequest> {
        self.pending.map(|pending| pending.request)
    }

    /// Return the number of WAIT T-states accumulated for the current request.
    pub fn wait_tstates(&self) -> Option<u32> {
        self.pending.map(|pending| pending.wait_tstates)
    }

    /// Extend the active bus cycle by one WAIT T-state.
    pub fn insert_wait_tstate(&mut self) -> Result<(), Z80BusHandshakeError> {
        let pending = self
            .pending
            .as_mut()
            .ok_or(Z80BusHandshakeError::NoRequestPending)?;
        pending.wait_tstates = pending
            .wait_tstates
            .checked_add(1)
            .ok_or(Z80BusHandshakeError::WaitCounterExhausted)?;
        Ok(())
    }

    /// Complete the matching request at the completion edge supplied by the scheduler.
    pub fn complete(
        &mut self,
        request_id: u64,
        data: Option<u8>,
    ) -> Result<Z80BusResponse, Z80BusHandshakeError> {
        let pending = self.pending.ok_or(Z80BusHandshakeError::NoRequestPending)?;
        if pending.request.id != request_id {
            return Err(Z80BusHandshakeError::WrongRequestId {
                expected: pending.request.id,
                received: request_id,
            });
        }
        let requires_data = matches!(
            pending.request.operation,
            Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead | Z80BusOperation::PortRead
        );
        if requires_data && data.is_none() {
            return Err(Z80BusHandshakeError::MissingReadData);
        }
        if !requires_data && data.is_some() {
            return Err(Z80BusHandshakeError::UnexpectedWriteData);
        }
        self.pending = None;
        Ok(Z80BusResponse { request_id, data })
    }

    /// Whether a transaction is outstanding.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{Z80BusHandshake, Z80BusHandshakeError, Z80BusOperation};

    #[test]
    fn request_identity_is_stable_while_wait_extends_cycle() {
        let mut bus = Z80BusHandshake::default();
        let request = bus.begin(Z80BusOperation::MemoryRead, 0x1234, 3).unwrap();
        bus.insert_wait_tstate().unwrap();
        bus.insert_wait_tstate().unwrap();
        assert_eq!(bus.pending_request(), Some(request));
        assert_eq!(bus.wait_tstates(), Some(2));
        assert_eq!(
            bus.complete(request.id, Some(0xA5)).unwrap().data,
            Some(0xA5)
        );
        assert!(!bus.is_pending());
    }

    #[test]
    fn duplicate_begin_does_not_replace_pending_request() {
        let mut bus = Z80BusHandshake::default();
        let first = bus.begin(Z80BusOperation::MemoryRead, 0x1000, 3).unwrap();
        assert_eq!(
            bus.begin(Z80BusOperation::MemoryWrite { value: 0x42 }, 0x1001, 3),
            Err(Z80BusHandshakeError::RequestAlreadyPending)
        );
        assert_eq!(bus.pending_request(), Some(first));
    }

    #[test]
    fn wrong_completion_id_keeps_original_request_pending() {
        let mut bus = Z80BusHandshake::default();
        let request = bus.begin(Z80BusOperation::PortRead, 0x00DC, 4).unwrap();
        assert_eq!(
            bus.complete(request.id + 1, Some(0xFF)),
            Err(Z80BusHandshakeError::WrongRequestId {
                expected: request.id,
                received: request.id + 1,
            })
        );
        assert_eq!(bus.pending_request(), Some(request));
    }

    #[test]
    fn read_requires_data_and_write_rejects_data() {
        let mut bus = Z80BusHandshake::default();
        let read = bus.begin(Z80BusOperation::OpcodeFetch, 0x0000, 4).unwrap();
        assert_eq!(
            bus.complete(read.id, None),
            Err(Z80BusHandshakeError::MissingReadData)
        );
        assert!(bus.is_pending());
        bus.complete(read.id, Some(0x00)).unwrap();

        let write = bus
            .begin(Z80BusOperation::MemoryWrite { value: 0x42 }, 0xC000, 3)
            .unwrap();
        assert_eq!(
            bus.complete(write.id, Some(0x42)),
            Err(Z80BusHandshakeError::UnexpectedWriteData)
        );
        assert!(bus.is_pending());
        bus.complete(write.id, None).unwrap();
    }

    #[test]
    fn wait_counter_overflow_is_reported_without_wrapping_or_saturating() {
        let mut bus = Z80BusHandshake::default();
        let request = bus.begin(Z80BusOperation::MemoryRead, 0x1234, 3).unwrap();
        bus.pending.as_mut().unwrap().wait_tstates = u32::MAX;

        assert_eq!(
            bus.insert_wait_tstate(),
            Err(Z80BusHandshakeError::WaitCounterExhausted)
        );
        assert_eq!(bus.pending_request(), Some(request));
        assert_eq!(bus.wait_tstates(), Some(u32::MAX));
    }

    #[test]
    fn operations_without_pending_request_are_rejected() {
        let mut bus = Z80BusHandshake::default();
        assert_eq!(
            bus.complete(0, Some(0)),
            Err(Z80BusHandshakeError::NoRequestPending)
        );
        assert_eq!(
            bus.insert_wait_tstate(),
            Err(Z80BusHandshakeError::NoRequestPending)
        );
    }
}
