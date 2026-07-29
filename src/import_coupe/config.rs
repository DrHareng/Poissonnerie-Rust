pub struct CoupeConfig {
    pub number: u8,
    pub spreadsheet_id: &'static str,
    pub name: &'static str,
    /// `quarters_direct` ou `round_of_16`.
    pub bracket_format: &'static str,
    /// Timestamp Unix approximatif (début des poules).
    pub started_at: u64,
    /// Timestamp Unix approximatif (finale).
    pub completed_at: u64,
}

const COUPES: &[CoupeConfig] = &[
    CoupeConfig {
        number: 5,
        spreadsheet_id: "1PsNqcURGyB4YpRDBt2X5z7nvrtivWFFujQUySmbPXZM",
        name: "Coupe de la Poissonnerie 5",
        bracket_format: "quarters_direct",
        started_at: 1_725_244_800, // 2024-09-02
        completed_at: 1_730_188_800, // 2024-11-05
    },
    CoupeConfig {
        number: 6,
        spreadsheet_id: "1zEJWCAq4Ch-tZqtoawJrgg-5by-_wlZzuvsVIpwZvdI",
        name: "Coupe de la Poissonnerie 6",
        bracket_format: "round_of_16",
        started_at: 1_736_688_000, // 2025-01-06
        completed_at: 1_739_548_800, // 2025-02-15
    },
    CoupeConfig {
        number: 7,
        spreadsheet_id: "1FUEJXHv1G8CdJdQZqGhzegInESw_WiI5KCiteJp4wVY",
        name: "Coupe de la Poissonnerie 7",
        bracket_format: "round_of_16",
        started_at: 1_744_003_200, // 2025-04-07
        completed_at: 1_747_382_400, // 2025-05-17
    },
    CoupeConfig {
        number: 8,
        spreadsheet_id: "1c04tgnrKm_Z6PqUHpcTYVjGFesMjshGipc1w5uYNXtw",
        name: "Coupe de la Poissonnerie 8",
        bracket_format: "round_of_16",
        started_at: 1_756_636_800, // 2025-09-01
        completed_at: 1_759_411_200, // 2025-10-03
    },
    CoupeConfig {
        number: 9,
        spreadsheet_id: "1PrGANAtLs88jnx00qM_akw1PJeIEFYeX3OOjhY5TxT4",
        name: "Coupe de la Poissonnerie 9",
        bracket_format: "round_of_16",
        started_at: 1_763_452_800, // 2025-12-08
        completed_at: 1_766_832_000, // 2026-01-28
    },
    CoupeConfig {
        number: 10,
        spreadsheet_id: "1RrTYaap4mhz7DPxDM5FGTP7PnnTzMTQT7uwgdYQyqfg",
        name: "Coupe de la Poissonnerie 10",
        bracket_format: "round_of_16",
        started_at: 1_775_059_200, // 2026-04-13
        completed_at: 1_778_169_600, // 2026-05-25
    },
];

pub fn coupe_config(number: u8) -> Option<&'static CoupeConfig> {
    COUPES.iter().find(|c| c.number == number)
}

pub fn available_coupes() -> &'static [CoupeConfig] {
    COUPES
}
