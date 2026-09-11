// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Strong Vultr identifiers and resource snapshots.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::VultrError;

const MAX_REGION_ID_BYTES: usize = 16;
const MAX_COUNTRY_CODE_BYTES: usize = 8;
const MAX_PLAN_ID_BYTES: usize = 64;
const MAX_VPC_ID_BYTES: usize = 64;
const MAX_INSTANCE_ID_BYTES: usize = 64;
const MAX_INSTANCE_TEMPLATE_ID_BYTES: usize = 64;
const MAX_SSH_KEY_ID_BYTES: usize = 64;
const MAX_FIREWALL_GROUP_ID_BYTES: usize = 64;
const MAX_STARTUP_SCRIPT_ID_BYTES: usize = 64;
const MAX_ISO_ID_BYTES: usize = 64;
const MAX_SNAPSHOT_ID_BYTES: usize = 64;
const MAX_VFS_ID_BYTES: usize = 64;
const MAX_LABEL_BYTES: usize = 128;
const MAX_HOSTNAME_BYTES: usize = 253;
const MAX_PUBLIC_IP_BYTES: usize = 64;
const MAX_PRIVATE_IP_BYTES: usize = 64;
const MAX_TAG_BYTES: usize = 64;
const MAX_CLOUD_INIT_USER_DATA_BYTES: usize = 65_536;
const MAX_REGISTRY_ID_BYTES: usize = 64;
const MAX_REGISTRY_NAME_BYTES: usize = 128;
const MAX_REGISTRY_URN_BYTES: usize = 256;
const MAX_REPOSITORY_NAME_BYTES: usize = 255;
const MAX_ARTIFACT_DIGEST_BYTES: usize = 255;

macro_rules! validated_string_type {
    ($name:ident, $max:ident, $allow:path, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Constructs a validated value.
            pub fn new(value: impl Into<String>) -> Result<Self, VultrError> {
                let value = value.into();
                let trimmed = value.trim();

                if trimmed.is_empty() {
                    return Err(VultrError::Empty);
                }

                if trimmed.len() > $max {
                    return Err(VultrError::TooLong);
                }

                if !trimmed.bytes().all($allow) {
                    return Err(VultrError::InvalidCharacter);
                }

                Ok(Self(trimmed.to_owned()))
            }

            /// Returns the validated value.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

validated_string_type!(
    VultrRegionId,
    MAX_REGION_ID_BYTES,
    is_lowercase_token_byte,
    "Vultr region identifier such as `ams`, `ewr`, or `lhr`."
);
validated_string_type!(
    VultrCountryCode,
    MAX_COUNTRY_CODE_BYTES,
    is_uppercase_token_byte,
    "Vultr country code as returned by the regions API."
);
validated_string_type!(
    VultrPlanId,
    MAX_PLAN_ID_BYTES,
    is_lowercase_token_byte,
    "Vultr compute plan identifier."
);
validated_string_type!(
    VultrVpcId,
    MAX_VPC_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr VPC or VPC 2.0 identifier."
);
validated_string_type!(
    VultrInstanceId,
    MAX_INSTANCE_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr instance identifier."
);
validated_string_type!(
    VultrInstanceTemplateId,
    MAX_INSTANCE_TEMPLATE_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr instance-template identifier."
);
validated_string_type!(
    VultrSshKeyId,
    MAX_SSH_KEY_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr SSH key identifier."
);
validated_string_type!(
    VultrFirewallGroupId,
    MAX_FIREWALL_GROUP_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr firewall group identifier."
);
validated_string_type!(
    VultrStartupScriptId,
    MAX_STARTUP_SCRIPT_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr startup script identifier."
);
validated_string_type!(
    VultrIsoId,
    MAX_ISO_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr ISO image identifier."
);
validated_string_type!(
    VultrSnapshotId,
    MAX_SNAPSHOT_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr snapshot identifier."
);
validated_string_type!(
    VultrVfsId,
    MAX_VFS_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr File Storage subscription identifier."
);
validated_string_type!(
    VultrLabel,
    MAX_LABEL_BYTES,
    is_safe_label_byte,
    "Operator-visible Vultr label."
);
validated_string_type!(
    VultrHostname,
    MAX_HOSTNAME_BYTES,
    is_hostname_byte,
    "Vultr instance hostname."
);
validated_string_type!(
    VultrPublicIpAddress,
    MAX_PUBLIC_IP_BYTES,
    is_ip_address_byte,
    "Public IP address reported by Vultr for an instance."
);
validated_string_type!(
    VultrPrivateIpAddress,
    MAX_PRIVATE_IP_BYTES,
    is_ip_address_byte,
    "Private IP address reported by Vultr for an instance."
);
validated_string_type!(VultrTag, MAX_TAG_BYTES, is_tag_byte, "Vultr resource tag.");
validated_string_type!(
    VultrContainerRegistryId,
    MAX_REGISTRY_ID_BYTES,
    is_uuidish_token_byte,
    "Vultr container-registry identifier."
);
validated_string_type!(
    VultrContainerRegistryName,
    MAX_REGISTRY_NAME_BYTES,
    is_safe_label_byte,
    "Vultr container-registry name."
);
validated_string_type!(
    VultrContainerRegistryUrn,
    MAX_REGISTRY_URN_BYTES,
    is_urn_byte,
    "Vultr container-registry URN."
);
validated_string_type!(
    VultrContainerRepositoryName,
    MAX_REPOSITORY_NAME_BYTES,
    is_repository_name_byte,
    "Vultr container-repository image name."
);
validated_string_type!(
    VultrContainerArtifactDigest,
    MAX_ARTIFACT_DIGEST_BYTES,
    is_digest_byte,
    "Vultr container-artifact digest."
);

/// Cloud-init user-data supplied to Vultr during instance creation.
///
/// This value is redacted in `Debug` because bootstrap payloads often include
/// registry credentials, private-network keys, or application environment.
#[derive(Clone, PartialEq, Eq, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(transparent)]
pub struct VultrCloudInitUserData(String);

impl VultrCloudInitUserData {
    /// Constructs validated cloud-init user-data.
    pub fn new(value: impl Into<String>) -> Result<Self, VultrError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(VultrError::Empty);
        }
        if value.len() > MAX_CLOUD_INIT_USER_DATA_BYTES {
            return Err(VultrError::TooLong);
        }
        if !value.starts_with("#cloud-config\n") && !value.starts_with("#!/") {
            return Err(VultrError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    /// Returns the raw cloud-init user-data.
    pub fn expose_as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for VultrCloudInitUserData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("VultrCloudInitUserData")
            .field(&"<redacted>")
            .finish()
    }
}

impl<'de> Deserialize<'de> for VultrCloudInitUserData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Vultr operating system identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct VultrOperatingSystemId(u32);

impl VultrOperatingSystemId {
    /// Constructs a validated Vultr operating system identifier.
    pub const fn new(value: u32) -> Result<Self, VultrError> {
        if value == 0 {
            return Err(VultrError::InvalidNumber);
        }

        Ok(Self(value))
    }

    /// Returns the Vultr API numeric OS identifier.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Vultr Marketplace application identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct VultrMarketplaceAppId(u32);

impl VultrMarketplaceAppId {
    /// Constructs a validated Marketplace application identifier.
    pub const fn new(value: u32) -> Result<Self, VultrError> {
        if value == 0 {
            return Err(VultrError::InvalidNumber);
        }
        Ok(Self(value))
    }

    /// Returns the provider numeric identifier.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for VultrMarketplaceAppId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Vultr Marketplace image identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct VultrMarketplaceImageId(u32);

impl VultrMarketplaceImageId {
    /// Constructs a validated Marketplace image identifier.
    pub const fn new(value: u32) -> Result<Self, VultrError> {
        if value == 0 {
            return Err(VultrError::InvalidNumber);
        }
        Ok(Self(value))
    }

    /// Returns the provider numeric identifier.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for VultrMarketplaceImageId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for VultrOperatingSystemId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Vultr instance lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VultrInstanceStatus {
    /// The instance is being provisioned or changed.
    Pending,
    /// The instance is running.
    Active,
    /// The instance is stopped.
    Stopped,
    /// The instance is suspended by provider policy.
    Suspended,
}

impl VultrInstanceStatus {
    /// Parses a Vultr API status into a stable enum.
    pub fn parse(value: &str) -> Result<Self, VultrError> {
        match value {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "stopped" => Ok(Self::Stopped),
            "suspended" => Ok(Self::Suspended),
            _ => Err(VultrError::UnknownInstanceStatus),
        }
    }
}

/// Vultr plan family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VultrPlanType {
    /// Regular cloud compute.
    CloudCompute,
    /// Dedicated cloud.
    DedicatedCloud,
    /// High frequency compute.
    HighFrequency,
    /// High performance compute.
    HighPerformance,
    /// Optimized cloud.
    OptimizedCloud,
    /// Bare metal.
    BareMetal,
    /// Cloud GPU.
    CloudGpu,
}

impl VultrPlanType {
    /// Parses a Vultr plan type code.
    pub fn parse(value: &str) -> Result<Self, VultrError> {
        match value {
            "vc2" => Ok(Self::CloudCompute),
            "vdc" => Ok(Self::DedicatedCloud),
            "vhf" => Ok(Self::HighFrequency),
            "vhp" => Ok(Self::HighPerformance),
            "voc" | "voc-g" | "voc-c" | "voc-m" | "voc-s" => Ok(Self::OptimizedCloud),
            "vbm" | "dedicated" => Ok(Self::BareMetal),
            "vcg" => Ok(Self::CloudGpu),
            _ => Err(VultrError::UnknownPlanType),
        }
    }
}

/// Vultr plan type code accepted by current Vultr plan filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VultrPlanTypeCode {
    /// All available instance plan types.
    All,
    /// Regular cloud compute.
    Vc2,
    /// Dedicated cloud.
    Vdc,
    /// High frequency compute.
    Vhf,
    /// High performance compute.
    Vhp,
    /// All optimized cloud types.
    Voc,
    /// General purpose optimized cloud.
    VocG,
    /// CPU optimized cloud.
    VocC,
    /// Memory optimized cloud.
    VocM,
    /// Storage optimized cloud.
    VocS,
    /// Bare metal.
    Vbm,
    /// Cloud GPU.
    Vcg,
}

impl VultrPlanTypeCode {
    /// Parses a Vultr plan type code.
    pub fn parse(value: &str) -> Result<Self, VultrError> {
        match value {
            "all" => Ok(Self::All),
            "vc2" => Ok(Self::Vc2),
            "vdc" => Ok(Self::Vdc),
            "vhf" => Ok(Self::Vhf),
            "vhp" => Ok(Self::Vhp),
            "voc" => Ok(Self::Voc),
            "voc-g" => Ok(Self::VocG),
            "voc-c" => Ok(Self::VocC),
            "voc-m" => Ok(Self::VocM),
            "voc-s" => Ok(Self::VocS),
            "vbm" => Ok(Self::Vbm),
            "vcg" => Ok(Self::Vcg),
            _ => Err(VultrError::UnknownPlanType),
        }
    }

    /// Returns the Vultr API filter code.
    pub const fn as_api_code(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Vc2 => "vc2",
            Self::Vdc => "vdc",
            Self::Vhf => "vhf",
            Self::Vhp => "vhp",
            Self::Voc => "voc",
            Self::VocG => "voc-g",
            Self::VocC => "voc-c",
            Self::VocM => "voc-m",
            Self::VocS => "voc-s",
            Self::Vbm => "vbm",
            Self::Vcg => "vcg",
        }
    }

    /// Returns the broader plan family represented by this type code.
    pub const fn family(self) -> Option<VultrPlanType> {
        match self {
            Self::All => None,
            Self::Vc2 => Some(VultrPlanType::CloudCompute),
            Self::Vdc => Some(VultrPlanType::DedicatedCloud),
            Self::Vhf => Some(VultrPlanType::HighFrequency),
            Self::Vhp => Some(VultrPlanType::HighPerformance),
            Self::Voc | Self::VocG | Self::VocC | Self::VocM | Self::VocS => {
                Some(VultrPlanType::OptimizedCloud)
            }
            Self::Vbm => Some(VultrPlanType::BareMetal),
            Self::Vcg => Some(VultrPlanType::CloudGpu),
        }
    }
}

/// Vultr datacenter airport codes known to ReallyMe policy at compile time.
///
/// Vultr can add regions independently of our release cycle, so API responses
/// must continue to use [`VultrRegionId`] as the source of truth. This enum is
/// only for curated UI choices and policy defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
#[serde(rename_all = "lowercase")]
pub enum VultrDatacenterCode {
    /// AMS.
    AMS,
    /// ATL.
    ATL,
    /// BLR.
    BLR,
    /// BOM.
    BOM,
    /// CDG.
    CDG,
    /// DEL.
    DEL,
    /// DFW.
    DFW,
    /// DAL legacy ReallyMe alias for the Dallas/Fort Worth datacenter.
    DAL,
    /// EWR.
    EWR,
    /// FRA.
    FRA,
    /// HNL.
    HNL,
    /// IAD.
    IAD,
    /// ICN.
    ICN,
    /// ITM.
    ITM,
    /// JNB.
    JNB,
    /// LAX.
    LAX,
    /// LHR.
    LHR,
    /// LON legacy ReallyMe alias for the London datacenter.
    LON,
    /// MAD.
    MAD,
    /// MAN.
    MAN,
    /// MEL.
    MEL,
    /// MEX.
    MEX,
    /// MIA.
    MIA,
    /// MXP.
    MXP,
    /// NRT.
    NRT,
    /// NYC legacy ReallyMe alias for the New Jersey datacenter.
    NYC,
    /// ORD.
    ORD,
    /// SAO.
    SAO,
    /// SCL.
    SCL,
    /// SEA.
    SEA,
    /// SGP.
    SGP,
    /// SIG legacy ReallyMe alias for the Singapore datacenter.
    SIG,
    /// SJC.
    SJC,
    /// SFO legacy ReallyMe alias for the Silicon Valley datacenter.
    SFO,
    /// STO.
    STO,
    /// SYD.
    SYD,
    /// TLV.
    TLV,
    /// WAW.
    WAW,
    /// YTO.
    YTO,
    /// YYZ legacy ReallyMe alias for the Toronto datacenter.
    YYZ,
}

impl VultrDatacenterCode {
    /// Parses a Vultr region id into a curated datacenter code.
    pub fn parse_id(value: &str) -> Option<Self> {
        match value {
            "ams" => Some(Self::AMS),
            "atl" => Some(Self::ATL),
            "blr" => Some(Self::BLR),
            "bom" => Some(Self::BOM),
            "cdg" => Some(Self::CDG),
            "del" => Some(Self::DEL),
            "dfw" => Some(Self::DFW),
            "ewr" => Some(Self::EWR),
            "fra" => Some(Self::FRA),
            "hnl" => Some(Self::HNL),
            "iad" => Some(Self::IAD),
            "icn" => Some(Self::ICN),
            "itm" => Some(Self::ITM),
            "jnb" => Some(Self::JNB),
            "lax" => Some(Self::LAX),
            "lhr" => Some(Self::LHR),
            "mad" => Some(Self::MAD),
            "man" => Some(Self::MAN),
            "mel" => Some(Self::MEL),
            "mex" => Some(Self::MEX),
            "mia" => Some(Self::MIA),
            "mxp" => Some(Self::MXP),
            "nrt" => Some(Self::NRT),
            "nyc" => Some(Self::NYC),
            "ord" => Some(Self::ORD),
            "sao" => Some(Self::SAO),
            "scl" => Some(Self::SCL),
            "sea" => Some(Self::SEA),
            "sgp" => Some(Self::SGP),
            "sjc" => Some(Self::SJC),
            "sto" => Some(Self::STO),
            "syd" => Some(Self::SYD),
            "tlv" => Some(Self::TLV),
            "waw" => Some(Self::WAW),
            "yto" => Some(Self::YTO),
            _ => None,
        }
    }

    /// Returns the Vultr region id used by the API.
    pub const fn id(self) -> &'static str {
        match self {
            Self::AMS => "ams",
            Self::ATL => "atl",
            Self::BLR => "blr",
            Self::BOM => "bom",
            Self::CDG => "cdg",
            Self::DEL => "del",
            Self::DFW => "dfw",
            Self::DAL => "dfw",
            Self::EWR => "ewr",
            Self::FRA => "fra",
            Self::HNL => "hnl",
            Self::IAD => "iad",
            Self::ICN => "icn",
            Self::ITM => "itm",
            Self::JNB => "jnb",
            Self::LAX => "lax",
            Self::LHR => "lhr",
            Self::LON => "lhr",
            Self::MAD => "mad",
            Self::MAN => "man",
            Self::MEL => "mel",
            Self::MEX => "mex",
            Self::MIA => "mia",
            Self::MXP => "mxp",
            Self::NRT => "nrt",
            Self::NYC => "ewr",
            Self::ORD => "ord",
            Self::SAO => "sao",
            Self::SCL => "scl",
            Self::SEA => "sea",
            Self::SGP => "sgp",
            Self::SIG => "sgp",
            Self::SJC => "sjc",
            Self::SFO => "sjc",
            Self::STO => "sto",
            Self::SYD => "syd",
            Self::TLV => "tlv",
            Self::WAW => "waw",
            Self::YTO => "yto",
            Self::YYZ => "yto",
        }
    }
}

/// Vultr region snapshot returned by the provider API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VultrRegion {
    id: VultrRegionId,
    datacenter_code: Option<VultrDatacenterCode>,
    city: String,
    country: VultrCountryCode,
    continent: String,
}

impl VultrRegion {
    /// Constructs a region snapshot after boundary validation.
    pub fn new(
        id: VultrRegionId,
        city: String,
        country: VultrCountryCode,
        continent: String,
    ) -> Result<Self, VultrError> {
        validate_public_text(&city, MAX_LABEL_BYTES)?;
        validate_public_text(&continent, MAX_LABEL_BYTES)?;
        let datacenter_code = VultrDatacenterCode::parse_id(id.as_str());
        Ok(Self {
            id,
            datacenter_code,
            city,
            country,
            continent,
        })
    }

    /// Returns the region identifier.
    pub const fn id(&self) -> &VultrRegionId {
        &self.id
    }

    /// Returns the curated airport-code value when this region is known to
    /// ReallyMe policy.
    pub const fn datacenter_code(&self) -> Option<VultrDatacenterCode> {
        self.datacenter_code
    }

    /// Returns the region city.
    pub fn city(&self) -> &str {
        self.city.as_str()
    }

    /// Returns the region country code.
    pub const fn country(&self) -> &VultrCountryCode {
        &self.country
    }

    /// Returns the region continent.
    pub fn continent(&self) -> &str {
        self.continent.as_str()
    }
}

/// Vultr plan snapshot returned by the provider API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VultrPlan {
    id: VultrPlanId,
    plan_type: VultrPlanType,
    vcpu_count: u32,
    ram_mb: u32,
    disk_gb: u32,
    monthly_cost_cents: u32,
}

impl VultrPlan {
    /// Constructs a plan snapshot after boundary validation.
    pub fn new(
        id: VultrPlanId,
        plan_type: VultrPlanType,
        vcpu_count: u32,
        ram_mb: u32,
        disk_gb: u32,
        monthly_cost_cents: u32,
    ) -> Result<Self, VultrError> {
        if vcpu_count == 0 || ram_mb == 0 || disk_gb == 0 {
            return Err(VultrError::InvalidNumber);
        }

        Ok(Self {
            id,
            plan_type,
            vcpu_count,
            ram_mb,
            disk_gb,
            monthly_cost_cents,
        })
    }

    /// Returns the plan identifier.
    pub const fn id(&self) -> &VultrPlanId {
        &self.id
    }

    /// Returns the plan family.
    pub const fn plan_type(&self) -> VultrPlanType {
        self.plan_type
    }

    /// Returns the virtual CPU count.
    pub const fn vcpu_count(&self) -> u32 {
        self.vcpu_count
    }

    /// Returns the RAM in megabytes.
    pub const fn ram_mb(&self) -> u32 {
        self.ram_mb
    }

    /// Returns the disk size in gigabytes.
    pub const fn disk_gb(&self) -> u32 {
        self.disk_gb
    }

    /// Returns the monthly cost in cents.
    pub const fn monthly_cost_cents(&self) -> u32 {
        self.monthly_cost_cents
    }
}

/// Vultr VPC snapshot returned by the provider API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VultrVpc {
    id: VultrVpcId,
    region: VultrRegionId,
    description: String,
    ip_block: String,
    prefix_length: u8,
}

impl VultrVpc {
    /// Constructs a VPC snapshot after boundary validation.
    pub fn new(
        id: VultrVpcId,
        region: VultrRegionId,
        description: String,
        ip_block: String,
        prefix_length: u8,
    ) -> Result<Self, VultrError> {
        validate_public_text(&description, MAX_LABEL_BYTES)?;
        validate_public_text(&ip_block, MAX_LABEL_BYTES)?;
        if prefix_length > 32 {
            return Err(VultrError::InvalidNumber);
        }

        Ok(Self {
            id,
            region,
            description,
            ip_block,
            prefix_length,
        })
    }

    /// Returns the VPC identifier.
    pub const fn id(&self) -> &VultrVpcId {
        &self.id
    }

    /// Returns the VPC region.
    pub const fn region(&self) -> &VultrRegionId {
        &self.region
    }

    /// Returns the VPC description.
    pub fn description(&self) -> &str {
        self.description.as_str()
    }

    /// Returns the private IPv4 block.
    pub fn ip_block(&self) -> &str {
        self.ip_block.as_str()
    }

    /// Returns the CIDR prefix length.
    pub const fn prefix_length(&self) -> u8 {
        self.prefix_length
    }
}

/// Vultr instance snapshot returned by the provider API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VultrInstance {
    id: VultrInstanceId,
    region: VultrRegionId,
    plan: VultrPlanId,
    label: VultrLabel,
    status: VultrInstanceStatus,
    main_ip: Option<VultrPublicIpAddress>,
    internal_ip: Option<VultrPrivateIpAddress>,
    vpc_ids: Vec<VultrVpcId>,
    vpc_only: bool,
}

/// Vultr instance-template snapshot returned by the provider API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VultrInstanceTemplate {
    id: VultrInstanceTemplateId,
    plan: VultrPlanId,
    label: Option<VultrLabel>,
    os_id: Option<VultrOperatingSystemId>,
    iso_id: Option<VultrIsoId>,
    snapshot_id: Option<VultrSnapshotId>,
    marketplace_app_id: Option<VultrMarketplaceAppId>,
    marketplace_image_id: Option<VultrMarketplaceImageId>,
    script_id: Option<VultrStartupScriptId>,
    ssh_key_ids: Vec<VultrSshKeyId>,
    vpc_ids: Vec<VultrVpcId>,
    vfs_ids: Vec<VultrVfsId>,
    user_data: Option<VultrCloudInitUserData>,
}

impl VultrInstanceTemplate {
    /// Constructs an instance-template snapshot after boundary validation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: VultrInstanceTemplateId,
        plan: VultrPlanId,
        label: Option<VultrLabel>,
        os_id: Option<VultrOperatingSystemId>,
        iso_id: Option<VultrIsoId>,
        snapshot_id: Option<VultrSnapshotId>,
        marketplace_app_id: Option<VultrMarketplaceAppId>,
        marketplace_image_id: Option<VultrMarketplaceImageId>,
        script_id: Option<VultrStartupScriptId>,
        ssh_key_ids: Vec<VultrSshKeyId>,
        vpc_ids: Vec<VultrVpcId>,
        vfs_ids: Vec<VultrVfsId>,
        user_data: Option<VultrCloudInitUserData>,
    ) -> Self {
        Self {
            id,
            plan,
            label,
            os_id,
            iso_id,
            snapshot_id,
            marketplace_app_id,
            marketplace_image_id,
            script_id,
            ssh_key_ids,
            vpc_ids,
            vfs_ids,
            user_data,
        }
    }

    /// Returns the template identifier.
    pub const fn id(&self) -> &VultrInstanceTemplateId {
        &self.id
    }

    /// Returns the plan identifier.
    pub const fn plan(&self) -> &VultrPlanId {
        &self.plan
    }

    /// Returns the operator-visible label when set.
    pub const fn label(&self) -> Option<&VultrLabel> {
        self.label.as_ref()
    }

    /// Returns the operating-system identifier when set.
    pub const fn os_id(&self) -> Option<VultrOperatingSystemId> {
        self.os_id
    }

    /// Returns the ISO identifier when set.
    pub const fn iso_id(&self) -> Option<&VultrIsoId> {
        self.iso_id.as_ref()
    }

    /// Returns the snapshot identifier when set.
    pub const fn snapshot_id(&self) -> Option<&VultrSnapshotId> {
        self.snapshot_id.as_ref()
    }

    /// Returns the Marketplace application identifier when set.
    pub const fn marketplace_app_id(&self) -> Option<VultrMarketplaceAppId> {
        self.marketplace_app_id
    }

    /// Returns the Marketplace image identifier when set.
    pub const fn marketplace_image_id(&self) -> Option<VultrMarketplaceImageId> {
        self.marketplace_image_id
    }

    /// Returns the startup script identifier when set.
    pub const fn script_id(&self) -> Option<&VultrStartupScriptId> {
        self.script_id.as_ref()
    }

    /// Returns installed SSH key ids.
    pub fn ssh_key_ids(&self) -> &[VultrSshKeyId] {
        self.ssh_key_ids.as_slice()
    }

    /// Returns attached VPC ids.
    pub fn vpc_ids(&self) -> &[VultrVpcId] {
        self.vpc_ids.as_slice()
    }

    /// Returns attached Vultr File Storage ids.
    pub fn vfs_ids(&self) -> &[VultrVfsId] {
        self.vfs_ids.as_slice()
    }

    /// Returns cloud-init user-data when set.
    pub const fn user_data(&self) -> Option<&VultrCloudInitUserData> {
        self.user_data.as_ref()
    }
}

/// Vultr container-registry subscription snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VultrContainerRegistry {
    id: VultrContainerRegistryId,
    name: VultrContainerRegistryName,
    public: bool,
    urn: VultrContainerRegistryUrn,
    storage_bytes_used: u64,
}

impl VultrContainerRegistry {
    /// Constructs a registry snapshot after boundary validation.
    pub const fn new(
        id: VultrContainerRegistryId,
        name: VultrContainerRegistryName,
        public: bool,
        urn: VultrContainerRegistryUrn,
        storage_bytes_used: u64,
    ) -> Self {
        Self {
            id,
            name,
            public,
            urn,
            storage_bytes_used,
        }
    }

    /// Returns the registry identifier.
    pub const fn id(&self) -> &VultrContainerRegistryId {
        &self.id
    }

    /// Returns the registry name.
    pub const fn name(&self) -> &VultrContainerRegistryName {
        &self.name
    }

    /// Returns whether the registry is publicly readable.
    pub const fn public(&self) -> bool {
        self.public
    }

    /// Returns the provider URN.
    pub const fn urn(&self) -> &VultrContainerRegistryUrn {
        &self.urn
    }

    /// Returns provider-reported storage bytes used.
    pub const fn storage_bytes_used(&self) -> u64 {
        self.storage_bytes_used
    }
}

/// One repository inside a Vultr container registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VultrContainerRepository {
    registry_id: VultrContainerRegistryId,
    name: VultrContainerRepositoryName,
    description: String,
    pull_count: u64,
    artifact_count: u32,
}

impl VultrContainerRepository {
    /// Constructs a repository snapshot after boundary validation.
    pub fn new(
        registry_id: VultrContainerRegistryId,
        name: VultrContainerRepositoryName,
        description: String,
        pull_count: u64,
        artifact_count: u32,
    ) -> Result<Self, VultrError> {
        if !description.trim().is_empty() {
            validate_public_text(&description, MAX_LABEL_BYTES)?;
        }

        Ok(Self {
            registry_id,
            name,
            description,
            pull_count,
            artifact_count,
        })
    }

    /// Returns the parent registry identifier.
    pub const fn registry_id(&self) -> &VultrContainerRegistryId {
        &self.registry_id
    }

    /// Returns the repository image name.
    pub const fn name(&self) -> &VultrContainerRepositoryName {
        &self.name
    }

    /// Returns the operator-visible description when set.
    pub fn description(&self) -> &str {
        self.description.as_str()
    }

    /// Returns the pull count.
    pub const fn pull_count(&self) -> u64 {
        self.pull_count
    }

    /// Returns the artifact count.
    pub const fn artifact_count(&self) -> u32 {
        self.artifact_count
    }
}

/// One image artifact or digest inside a Vultr repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VultrContainerArtifact {
    registry_id: VultrContainerRegistryId,
    repository: VultrContainerRepositoryName,
    digest: VultrContainerArtifactDigest,
    tags: Vec<VultrTag>,
    size_bytes: u64,
}

impl VultrContainerArtifact {
    /// Constructs an artifact snapshot after boundary validation.
    pub fn new(
        registry_id: VultrContainerRegistryId,
        repository: VultrContainerRepositoryName,
        digest: VultrContainerArtifactDigest,
        tags: Vec<VultrTag>,
        size_bytes: u64,
    ) -> Self {
        Self {
            registry_id,
            repository,
            digest,
            tags,
            size_bytes,
        }
    }

    /// Returns the parent registry identifier.
    pub const fn registry_id(&self) -> &VultrContainerRegistryId {
        &self.registry_id
    }

    /// Returns the parent repository name.
    pub const fn repository(&self) -> &VultrContainerRepositoryName {
        &self.repository
    }

    /// Returns the OCI digest.
    pub const fn digest(&self) -> &VultrContainerArtifactDigest {
        &self.digest
    }

    /// Returns tags currently pointing at the artifact.
    pub fn tags(&self) -> &[VultrTag] {
        self.tags.as_slice()
    }

    /// Returns provider-reported size in bytes.
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

/// Private-network and address data attached to a Vultr instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VultrInstanceNetwork {
    main_ip: Option<VultrPublicIpAddress>,
    internal_ip: Option<VultrPrivateIpAddress>,
    vpc_ids: Vec<VultrVpcId>,
    vpc_only: bool,
}

impl VultrInstanceNetwork {
    /// Constructs an instance network snapshot.
    pub const fn new(
        main_ip: Option<VultrPublicIpAddress>,
        internal_ip: Option<VultrPrivateIpAddress>,
        vpc_ids: Vec<VultrVpcId>,
        vpc_only: bool,
    ) -> Self {
        Self {
            main_ip,
            internal_ip,
            vpc_ids,
            vpc_only,
        }
    }

    /// Returns the public IP address once Vultr has assigned one.
    pub const fn main_ip(&self) -> Option<&VultrPublicIpAddress> {
        self.main_ip.as_ref()
    }

    /// Returns the private IP address once Vultr has assigned one.
    pub const fn internal_ip(&self) -> Option<&VultrPrivateIpAddress> {
        self.internal_ip.as_ref()
    }

    /// Returns VPC attachments known for the instance.
    pub fn vpc_ids(&self) -> &[VultrVpcId] {
        self.vpc_ids.as_slice()
    }

    /// Returns whether the instance is private-network only.
    pub const fn vpc_only(&self) -> bool {
        self.vpc_only
    }
}

impl VultrInstance {
    /// Constructs an instance snapshot after boundary validation.
    pub fn new(
        id: VultrInstanceId,
        region: VultrRegionId,
        plan: VultrPlanId,
        label: VultrLabel,
        status: VultrInstanceStatus,
        network: VultrInstanceNetwork,
    ) -> Self {
        Self {
            id,
            region,
            plan,
            label,
            status,
            main_ip: network.main_ip,
            internal_ip: network.internal_ip,
            vpc_ids: network.vpc_ids,
            vpc_only: network.vpc_only,
        }
    }

    /// Returns the instance identifier.
    pub const fn id(&self) -> &VultrInstanceId {
        &self.id
    }

    /// Returns the instance region.
    pub const fn region(&self) -> &VultrRegionId {
        &self.region
    }

    /// Returns the instance plan.
    pub const fn plan(&self) -> &VultrPlanId {
        &self.plan
    }

    /// Returns the operator-visible label.
    pub const fn label(&self) -> &VultrLabel {
        &self.label
    }

    /// Returns the provider lifecycle status.
    pub const fn status(&self) -> VultrInstanceStatus {
        self.status
    }

    /// Returns the public IP address once Vultr has assigned one.
    pub const fn main_ip(&self) -> Option<&VultrPublicIpAddress> {
        self.main_ip.as_ref()
    }

    /// Returns the private IP address once Vultr has assigned one.
    pub const fn internal_ip(&self) -> Option<&VultrPrivateIpAddress> {
        self.internal_ip.as_ref()
    }

    /// Returns VPC attachments known for the instance.
    pub fn vpc_ids(&self) -> &[VultrVpcId] {
        self.vpc_ids.as_slice()
    }

    /// Returns whether the instance is private-network only.
    pub const fn vpc_only(&self) -> bool {
        self.vpc_only
    }
}

fn validate_public_text(value: &str, max_bytes: usize) -> Result<(), VultrError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(VultrError::Empty);
    }
    if trimmed.len() > max_bytes {
        return Err(VultrError::TooLong);
    }
    if !trimmed.bytes().all(is_safe_label_byte) {
        return Err(VultrError::InvalidCharacter);
    }
    Ok(())
}

fn is_lowercase_token_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-')
}

fn is_uppercase_token_byte(byte: u8) -> bool {
    byte.is_ascii_uppercase()
}

fn is_uuidish_token_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-')
}

fn is_hostname_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
}

fn is_ip_address_byte(byte: u8) -> bool {
    byte.is_ascii_hexdigit() || matches!(byte, b'.' | b':')
}

fn is_tag_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
}

fn is_repository_name_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.' | b'/')
}

fn is_digest_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b':' | b'-' | b'_')
}

fn is_urn_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
}

fn is_safe_label_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b' ' | b'-' | b'_' | b'.' | b':' | b'/' | b'(' | b')' | b','
        )
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "fixed test fixtures should fail loudly if validation invariants change"
)]
mod tests {
    use super::{VultrDatacenterCode, VultrError, VultrPlanId, VultrPlanTypeCode, VultrRegionId};

    #[test]
    fn region_id_rejects_empty_values() {
        assert_eq!(VultrRegionId::new(" "), Err(VultrError::Empty));
    }

    #[test]
    fn plan_id_rejects_uppercase_values() {
        assert_eq!(
            VultrPlanId::new("VC2-1C-1GB"),
            Err(VultrError::InvalidCharacter)
        );
    }

    #[test]
    fn plan_id_accepts_current_vultr_shape() {
        let value = VultrPlanId::new("vc2-1c-2gb").expect("valid plan id fixture");
        assert_eq!(value.as_str(), "vc2-1c-2gb");
    }

    #[test]
    fn datacenter_code_maps_current_vultr_ids() {
        const CURRENT_VULTR_REGION_IDS: &[&str] = &[
            "ams", "atl", "blr", "ord", "dfw", "del", "fra", "hnl", "jnb", "lhr", "lax", "mad",
            "man", "mel", "mex", "mia", "mxp", "bom", "ewr", "itm", "cdg", "scl", "sao", "sea",
            "icn", "sjc", "sgp", "sto", "syd", "tlv", "nrt", "yto", "waw",
        ];

        for region_id in CURRENT_VULTR_REGION_IDS {
            assert!(
                VultrDatacenterCode::parse_id(region_id).is_some(),
                "missing Vultr region id fixture: {region_id}"
            );
        }

        assert_eq!(
            VultrDatacenterCode::parse_id("mxp"),
            Some(VultrDatacenterCode::MXP)
        );
        assert_eq!(VultrDatacenterCode::DFW.id(), "dfw");
        assert_eq!(VultrDatacenterCode::EWR.id(), "ewr");
        assert_eq!(VultrDatacenterCode::SJC.id(), "sjc");
        assert_eq!(VultrDatacenterCode::YTO.id(), "yto");
        assert_eq!(VultrDatacenterCode::parse_id("future-region"), None);
    }

    #[test]
    fn datacenter_code_preserves_legacy_reallyme_aliases() {
        assert_eq!(VultrDatacenterCode::LON.id(), "lhr");
        assert_eq!(VultrDatacenterCode::NYC.id(), "ewr");
        assert_eq!(VultrDatacenterCode::SIG.id(), "sgp");
        assert_eq!(VultrDatacenterCode::DAL.id(), "dfw");
        assert_eq!(VultrDatacenterCode::SFO.id(), "sjc");
        assert_eq!(VultrDatacenterCode::YYZ.id(), "yto");
    }

    #[test]
    fn plan_type_code_parses_current_vultr_filters() {
        assert_eq!(
            VultrPlanTypeCode::parse("voc-g").expect("valid optimized plan code"),
            VultrPlanTypeCode::VocG
        );
        assert_eq!(VultrPlanTypeCode::Vhp.as_api_code(), "vhp");
        assert!(VultrPlanTypeCode::All.family().is_none());
    }
}
