// Copyright 2023, The Android Open Source Project
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

//! Wrappers around hypervisor back-ends.

extern crate alloc;

mod common;
mod gunyah;
mod kvm;

use crate::error::{Error, Result};
use alloc::boxed::Box;
pub use common::Hypervisor;
pub use common::HypervisorCap;
use gunyah::GunyahHypervisor;
pub use kvm::KvmError;
use kvm::KvmHypervisor;
use once_cell::race::OnceBox;
use psci::smccc::hvc64;
use uuid::Uuid;

enum HypervisorBackend {
    Kvm,
    Gunyah,
}

impl HypervisorBackend {
    fn get_hypervisor(&self) -> &'static dyn Hypervisor {
        match self {
            Self::Kvm => &KvmHypervisor,
            Self::Gunyah => &GunyahHypervisor,
        }
    }
}

impl TryFrom<Uuid> for HypervisorBackend {
    type Error = Error;

    fn try_from(uuid: Uuid) -> Result<HypervisorBackend> {
        match uuid {
            GunyahHypervisor::UUID => Ok(HypervisorBackend::Gunyah),
            KvmHypervisor::UUID => Ok(HypervisorBackend::Kvm),
            u => Err(Error::UnsupportedHypervisorUuid(u)),
        }
    }
}

const ARM_SMCCC_VENDOR_HYP_CALL_UID_FUNC_ID: u32 = 0x8600ff01;

fn query_vendor_hyp_call_uid() -> Uuid {
    let args = [0u64; 17];
    let res = hvc64(ARM_SMCCC_VENDOR_HYP_CALL_UID_FUNC_ID, args);

    // KVM's UUID of "28b46fb6-2ec5-11e9-a9ca-4b564d003a74" is generated by
    // Uuid::from_u128() from an input value of
    // 0x28b46fb6_2ec511e9_a9ca4b56_4d003a74. ARM's SMC calling convention
    // (Document number ARM DEN 0028E) describes the UUID register mapping such
    // that W0 contains bytes 0..3 of UUID, with byte 0 in lower order bits. In
    // the KVM example, byte 0 of KVM's UUID (0x28) will be returned in the low
    // 8-bits of W0, while byte 15 (0x74) will be returned in bits 31-24 of W3.
    //
    // `uuid` value derived below thus need to be byte-reversed before
    // being used in Uuid::from_u128(). Alternately use Uuid::from_u128_le()
    // to achieve the same.

    let uuid = ((res[3] as u32 as u128) << 96)
        | ((res[2] as u32 as u128) << 64)
        | ((res[1] as u32 as u128) << 32)
        | (res[0] as u32 as u128);

    Uuid::from_u128_le(uuid)
}

fn detect_hypervisor() -> HypervisorBackend {
    query_vendor_hyp_call_uid().try_into().expect("Unknown hypervisor")
}

/// Gets the hypervisor singleton.
pub fn get_hypervisor() -> &'static dyn Hypervisor {
    static HYPERVISOR: OnceBox<HypervisorBackend> = OnceBox::new();

    HYPERVISOR.get_or_init(|| Box::new(detect_hypervisor())).get_hypervisor()
}
