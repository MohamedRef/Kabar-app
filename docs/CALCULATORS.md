# Calculators

Generated from `crates/core/src/calc` (run `cargo test -p batna-core -- --ignored print_reference`). A `calc` item names one of these in `calculator:` and gives its `inputs` by key (a key may drop its unit suffix: `alt` for `alt_u_l`). `just validate` computes every item, so a typo, a missing input or an `expect` that disagrees fails the check. `expect` may be the band id (`child_pugh.b`) or text that contains the result (`Child-Pugh B (8 points)`, `Stage C`).

## `child_pugh`: Child-Pugh

Source: Book Lecture 1, Table 2 (p. 14); INR bands: Pugh 1973 as used by AASLD

| input | kind | |
|---|---|---|
| `encephalopathy` | `none`, `grade_1_2`, `grade_3_4` | Encephalopathy |
| `ascites` | `absent`, `mild_moderate`, `severe` | Ascites |
| `bilirubin_mg_dl` | number, mg/dL | Bilirubin |
| `albumin_g_dl` | number, g/dL | Albumin |
| `pt_prolonged_s` | number, s | Prothrombin time, seconds prolonged; optional |
| `inr` | number | INR; optional |

## `meld`: MELD

Source: UNOS/OPTN MELD (Kamath 2001); book Lecture 1, p. 14–15

| input | kind | |
|---|---|---|
| `bilirubin_mg_dl` | number, mg/dL | Bilirubin |
| `inr` | number | INR |
| `creatinine_mg_dl` | number, mg/dL | Creatinine |
| `dialysis` | true / false (default false) | Dialysis twice in the past week |

## `meld_na`: MELD-Na

Source: UNOS/OPTN MELD-Na 2016 (Kim 2008); book Lecture 1, p. 14–15

| input | kind | |
|---|---|---|
| `bilirubin_mg_dl` | number, mg/dL | Bilirubin |
| `inr` | number | INR |
| `creatinine_mg_dl` | number, mg/dL | Creatinine |
| `dialysis` | true / false (default false) | Dialysis twice in the past week |
| `sodium_mmol_l` | number, mmol/L | Sodium |

## `albi`: ALBI grade

Source: Johnson 2015; book Lecture 18, p. 363

| input | kind | |
|---|---|---|
| `bilirubin_mg_dl` | number, mg/dL | Bilirubin |
| `albumin_g_dl` | number, g/dL | Albumin |

## `fib4`: FIB-4

Source: Sterling 2006; book Lecture 6, p. 102

| input | kind | |
|---|---|---|
| `age_years` | number, years | Age |
| `ast_u_l` | number, U/L | AST |
| `alt_u_l` | number, U/L | ALT |
| `platelets_10e9_l` | number, ×10⁹/L | Platelets |

## `apri`: APRI

Source: Wai 2003; WHO HCV guidelines 2016 (book Lecture 1, p. 10 names the test)

| input | kind | |
|---|---|---|
| `ast_u_l` | number, U/L | AST |
| `ast_uln_u_l` | number, U/L | AST upper limit of normal |
| `platelets_10e9_l` | number, ×10⁹/L | Platelets |

## `r_ratio`: R ratio (DILI pattern)

Source: EASL DILI 2019; Kullak-Ublick 2017 (book Lecture 8, p. 129)

| input | kind | |
|---|---|---|
| `alt_u_l` | number, U/L | ALT |
| `alt_uln_u_l` | number, U/L | ALT upper limit of normal |
| `alp_u_l` | number, U/L | ALP |
| `alp_uln_u_l` | number, U/L | ALP upper limit of normal |

## `maddrey`: Maddrey discriminant function

Source: Maddrey 1978, Carithers 1989; AASLD 2019

| input | kind | |
|---|---|---|
| `pt_s` | number, s | Prothrombin time (patient) |
| `pt_control_s` | number, s | Prothrombin time (control) |
| `bilirubin_mg_dl` | number, mg/dL | Bilirubin |

## `glasgow_ah`: Glasgow alcoholic hepatitis score

Source: Forrest 2005 (Gut)

| input | kind | |
|---|---|---|
| `age_years` | number, years | Age |
| `wbc_10e9_l` | number, ×10⁹/L | White cells |
| `urea_mmol_l` | number, mmol/L | Urea |
| `inr` | number | INR or PT ratio |
| `bilirubin_mg_dl` | number, mg/dL | Bilirubin |

## `saag`: SAAG

Source: Book Lecture 9, p. 150–151

| input | kind | |
|---|---|---|
| `serum_albumin_g_dl` | number, g/dL | Serum albumin |
| `ascitic_albumin_g_dl` | number, g/dL | Ascitic albumin |
| `ascitic_protein_g_dl` | number, g/dL | Ascitic total protein; optional |

## `ascitic_pmn`: Ascitic PMN / SBP

Source: Book Lecture 9, p. 167–168

| input | kind | |
|---|---|---|
| `pmn_per_mm3` | number, /mm³ | Ascitic PMN (neutrophils) |
| `culture` | `pending`, `negative`, `monomicrobial`, `polymicrobial` | Ascitic culture |
| `protein_g_dl` | number, g/dL | Ascitic total protein; optional |
| `glucose_mg_dl` | number, mg/dL | Ascitic glucose; optional |
| `ldh_above_serum` | true / false (default false) | Ascitic LDH above serum LDH |

## `bclc`: BCLC staging

Source: Book Lecture 18, p. 362 and 364–373 (BCLC 2018)

| input | kind | |
|---|---|---|
| `performance_status` | number | ECOG performance status |
| `child_pugh_points` | number | Child-Pugh points |
| `ascites` | true / false (default false) | Ascites present |
| `nodules` | number | Number of nodules |
| `largest_cm` | number, cm | Largest nodule |
| `vascular_invasion` | true / false (default false) | Macrovascular invasion |
| `extrahepatic` | true / false (default false) | Extrahepatic spread |

## `kings_college`: King's College criteria

Source: Book Lecture 10, p. 187 (O'Grady 1989; Bernal 2002)

| input | kind | |
|---|---|---|
| `cause` | `acetaminophen`, `wilson_budd_chiari`, `indeterminate_or_drug`, `other` | Cause of ALF |
| `inr` | number | INR |
| `ph` | number | Arterial pH (after resuscitation); optional |
| `lactate_mmol_l` | number, mmol/L | Lactate; optional |
| `creatinine_mg_dl` | number, mg/dL | Creatinine; optional |
| `he_stage` | number | Encephalopathy stage; optional |
| `bilirubin_mg_dl` | number, mg/dL | Bilirubin; optional |
| `age_years` | number, years | Age; optional |
| `jaundice_to_he_days` | number, days | Days from jaundice to encephalopathy; optional |
| `over_24h` | true / false (default false) | More than 24 h since ingestion |

## `clichy`: Clichy criteria

Source: Book Lecture 10, p. 187 (Bernuau 1986)

| input | kind | |
|---|---|---|
| `he_stage` | number | Encephalopathy stage |
| `factor_v_pct` | number, % of normal | Factor V |
| `age_years` | number, years | Age |

## `ranson`: Ranson's criteria

Source: Ranson 1974/1979; book Lecture 15, p. 288 (mortality bands)

| input | kind | |
|---|---|---|
| `gallstone` | true / false (default false) | Gallstone pancreatitis |
| `age_years` | number, years | Age |
| `wbc_per_mm3` | number, /mm³ | White cells |
| `glucose_mg_dl` | number, mg/dL | Glucose |
| `ldh_iu_l` | number, IU/L | LDH |
| `ast_iu_l` | number, IU/L | AST |
| `hct_fall_points` | number, points | Haematocrit fall in 48 h; optional |
| `bun_rise_mg_dl` | number, mg/dL | BUN rise in 48 h; optional |
| `calcium_mg_dl` | number, mg/dL | Calcium at 48 h; optional |
| `pao2_mm_hg` | number, mmHg | PaO₂ at 48 h; optional |
| `base_deficit_meq_l` | number, mEq/L | Base deficit at 48 h; optional |
| `fluid_sequestration_l` | number, L | Fluid sequestration in 48 h; optional |

## `bisap`: BISAP

Source: Wu 2008; book Lecture 15, p. 288

| input | kind | |
|---|---|---|
| `bun_mg_dl` | number, mg/dL | BUN |
| `impaired_mental_status` | true / false (default false) | Impaired mental status |
| `sirs` | true / false (default false) | SIRS (2 or more criteria) |
| `age_years` | number, years | Age |
| `pleural_effusion` | true / false (default false) | Pleural effusion |
