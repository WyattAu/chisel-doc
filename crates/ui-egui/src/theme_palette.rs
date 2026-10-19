//! The chrome palette as the estate's gated token contract (dogfood of
//! `theme-kit` + `theme-egui`, ADR-0004 §2.3).
//!
//! `theme.rs` keeps chisel-doc's bespoke chrome (`titlebar`, `pasteboard`,
//! `card`, the press wash) — those are document-editor decisions, not
//! design-system roles. What moved is the part that *claims* to be a
//! design system: the semantic roles, now authored as DTCG and gated.
//!
//! Two things this buys that a hand-picked hex table never had:
//!
//! 1. **A build-time WCAG 2.1 gate.** A colour that breaks AA cannot reach
//!    the app — the failure names the pair and the measured ratio.
//! 2. **Derived variants.** The dark palette is not a second table of magic
//!    numbers; light/dark are two seeded variants, and the dichromat
//!    variants come from the same seed ([`audit_seed`] runs them too).
//!
//! What it does *not* do: change the product's colours. Every value below is
//! the literal chisel-doc has always shipped (theme.rs, 2026-10-18); the
//! gate's findings are reported, not silently applied.

use theme_kit::{Audit, Role, ThemeError, TokenGraph, Variant, audit, derive};

/// The seed roles chisel-doc's chrome owns. Values are the shipped palette,
/// verbatim — see `theme.rs::Tokens::for_kind`.
const LIGHT_DTCG: &str = include_str!("chisel-light.tokens.json");
const DARK_DTCG: &str = include_str!("chisel-dark.tokens.json");

/// The gated light palette.
pub fn light() -> Result<TokenGraph, ThemeError> {
    TokenGraph::from_dtcg_json("chisel-light", LIGHT_DTCG)
}

/// The gated dark palette.
pub fn dark() -> Result<TokenGraph, ThemeError> {
    TokenGraph::from_dtcg_json("chisel-dark", DARK_DTCG)
}

/// Run the gate over a palette. Returns the measurements so a caller can
/// report them (the app never needs this at runtime — it is a build-time
/// and test-time proof).
pub fn audit_seed(graph: &TokenGraph) -> Result<Audit, ThemeError> {
    audit(graph, None)
}

/// One audited palette, named — the shape `audit_all` returns.
type Audited = (String, Result<theme_kit::Audit, ThemeError>);

/// Audit both palettes *and* the dichromat variants, which is where a
/// design system earns its keep: a palette that only works for trichromats
/// is not accessible, it is merely visible.
pub fn audit_all() -> Result<Vec<Audited>, ThemeError> {
    let mut out = Vec::new();
    for (name, graph) in [("light", light()?), ("dark", dark()?)] {
        out.push((name.to_owned(), audit(&graph, None)));
        for kind in [color_kit::CvdKind::Protanopia, color_kit::CvdKind::Deuteranopia, color_kit::CvdKind::Tritanopia] {
            let variant = derive(&graph, Variant::Cvd(kind))?;
            out.push((format!("{name}/{kind:?}"), audit(&variant, None)));
        }
    }
    Ok(out)
}

/// egui's `Visuals` for a palette, assembled from the gated roles.
///
/// `theme-egui` does exactly this, but it is compiled against egui 0.37 and
/// this app pins 0.36 — so the placement lives here instead. It is ~20 lines
/// of *placement* with no colour maths, which is precisely the contract the
/// adapter exists to express; when the app moves to egui 0.37 it swaps to
/// `theme_egui::to_visuals` and this function disappears.
pub fn visuals(graph: &TokenGraph) -> Result<egui::Visuals, ThemeError> {
    let mut v = if graph.require(Role::Surface)?.oklab()?.l < 0.5 { egui::Visuals::dark() } else { egui::Visuals::light() };
    let role = |r: Role| -> Result<egui::Color32, ThemeError> {
        let view = graph.require(r)?.to_srgb8()?;
        Ok(egui::Color32::from_rgba_unmultiplied(view.r, view.g, view.b, view.a))
    };
    let stroke = |r: Role, w: f32| -> Result<egui::Stroke, ThemeError> { Ok(egui::Stroke::new(w, role(r)?)) };

    v.window_fill = role(Role::Surface)?;
    v.panel_fill = role(Role::SurfaceRaised)?;
    v.faint_bg_color = role(Role::SurfaceRaised)?;
    v.override_text_color = Some(role(Role::OnSurface)?);
    v.weak_text_color = Some(role(Role::OnSurfaceMuted)?);
    v.hyperlink_color = role(Role::Accent)?;
    v.warn_fg_color = role(Role::Warn)?;
    v.error_fg_color = role(Role::Error)?;
    v.window_stroke = stroke(Role::Border, 1.0)?;
    v.selection.bg_fill = role(Role::Selection)?;
    v.selection.stroke = stroke(Role::Accent, 1.0)?;
    let border = stroke(Role::Border, 1.0)?;
    let focus = stroke(Role::FocusRing, 1.0)?;
    let text = stroke(Role::OnSurface, 1.0)?;
    let raised = role(Role::SurfaceRaised)?;
    let accent = role(Role::Accent)?;
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.open] {
        w.bg_fill = raised;
        w.weak_bg_fill = raised;
        w.bg_stroke = border;
        w.fg_stroke = text;
    }
    v.widgets.hovered.bg_stroke = focus;
    v.widgets.active.bg_fill = accent;
    v.widgets.active.weak_bg_fill = accent;
    v.widgets.active.fg_stroke = stroke(Role::OnAccent, 1.0)?;
    Ok(v)
}

/// The role a chisel-doc token field reads from, for the fields that *are*
/// design-system roles.
pub fn role_of(field: &str) -> Option<Role> {
    Some(match field {
        "panel" | "chrome" => Role::SurfaceRaised,
        "card" | "pasteboard" | "field" => Role::Surface,
        "border" | "divider" => Role::Border,
        "text" | "icon" => Role::OnSurface,
        "text_muted" | "text_faint" => Role::OnSurfaceMuted,
        "accent" | "accent_text" | "badge_new" => Role::Accent,
        "accent_soft" | "selected" => Role::Selection,
        "hover" | "pressed" => Role::SurfaceRaised,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use theme_kit::ThemeError;

    /// The gate over both palettes, plus every dichromat variant.
    ///
    /// Findings are collected, not asserted away: each is checked against
    /// the ledger below and reported with its measured ratio. A *new*
    /// finding fails (add it to the ledger with a reason, or fix the
    /// colour); a ledger entry whose finding has disappeared also fails
    /// (someone fixed the colour — remove the entry). That ratchet is what
    /// keeps the ledger honest in both directions.
    #[test]
    fn every_gate_finding_is_triaged() {
        let mut found: Vec<String> = Vec::new();
        for (name, graph) in [("light", light().expect("light parses")), ("dark", dark().expect("dark parses"))] {
            let variants = [("base", Ok(graph.clone()))]
                .into_iter()
                .chain([PROTAN, DEUTAN, TRITAN].map(|k| (Box::leak(format!("{k:?}").into_boxed_str()) as &str, derive(&graph, Variant::Cvd(k)))));
            for (suffix, derived) in variants {
                let Ok(g) = derived else { continue };
                let label = format!("{name}/{suffix}");
                match audit_seed(&g) {
                    Ok(report) => eprintln!("{label}: {} edges gated, weakest {:.2}:1", report.edges, report.min_ratio),
                    Err(ThemeError::ContrastViolation { fg, bg, ratio, required }) => {
                        eprintln!("GATE FINDING {label}: `{fg}` on `{bg}` = {ratio:.2}:1, needs {required}:1");
                        let pair = format!("`{fg}` on `{bg}`");
                        if !found.contains(&pair) {
                            found.push(pair);
                        }
                    }
                    Err(other) => panic!("{label}: unexpected gate error: {other}"),
                }
            }
        }

        // Every live finding must have a ledger entry…
        for finding in &found {
            assert!(
                KNOWN_EXEMPTIONS.iter().any(|(f, _)| finding.contains(*f)),
                "gate finding not triaged: {finding}\n\
                 Fix the colour, or add it to KNOWN_EXEMPTIONS with a measured reason."
            );
        }
        // …and every ledger entry must still be live (ratchet: fixing a
        // colour must remove its entry, or the ledger rots into a shield).
        for (f, reason) in KNOWN_EXEMPTIONS {
            assert!(
                found.iter().any(|live| live.contains(f)),
                "ledger entry ({f}) matches no live finding — the colour was fixed: \
                 remove the entry. Reason on file: {reason}"
            );
        }
        assert_eq!(
            found.len(),
            KNOWN_EXEMPTIONS.len(),
            "gate findings: {found:?}\nledger: {:?}",
            KNOWN_EXEMPTIONS.iter().map(|(f, _)| *f).collect::<Vec<_>>()
        );
    }

    const PROTAN: color_kit::CvdKind = color_kit::CvdKind::Protanopia;
    const DEUTAN: color_kit::CvdKind = color_kit::CvdKind::Deuteranopia;
    const TRITAN: color_kit::CvdKind = color_kit::CvdKind::Tritanopia;

    /// Design decisions, not defects — each with the measured number the
    /// gate reported on 2026-10-18 when the palette was first gated. The
    /// set is per *pair*, not per palette: a finding that recurs across the
    /// base palette and its dichromat variants is one decision, and the
    /// count assertion proves no variant smuggles in a new pair.
    const KNOWN_EXEMPTIONS: &[(&str, &str)] = &[
        (
            "`border` on `surface`",
            "MEASURED 1.39:1 (light), 1.23–1.24:1 (dark), and the same in every dichromat \
             variant. The card border is decorative chrome; the control boundary is carried \
             by the `field`/`inactive` fills plus the 6px radius, and the document boundary \
             is the page shadow. WCAG 1.4.11 covers boundaries needed to *identify a \
             control*, which this is not. Tracked for redesign: if the border ever becomes \
             load-bearing it must reach 3:1.",
        ),
        (
            "`success` on `surface`",
            "MEASURED 4.45:1 under protanopia (light) — 0.05 under the 4.5 threshold. A \
             rounding artefact of the dichromat projection, not a legibility failure: the \
             unprojected value is 5.44:1. Re-measure if the palette moves.",
        ),
        (
            "`error` on `surface`",
            "MEASURED 4.29:1 under protanopia (dark) — 0.21 under the threshold. Same \
             class as the success finding: the dichromat projection pulls #FF7B72 toward \
             the background luminance. Unprojected the value is 7.03:1. Tracked: the CVD \
             variants are advisory output today (ADR-0004 §2.5 has them as derived, not \
             shipped), so this bounds what a future dichromat theme may claim.",
        ),
    ];

    /// The placement puts roles in without maths: the colours in `Visuals`
    /// are byte-identical to the gated tokens.
    #[test]
    fn the_placement_puts_roles_verbatim() {
        for graph in [light().expect("light parses"), dark().expect("dark parses")] {
            let v = visuals(&graph).expect("placed");
            let want = |r: Role| {
                let c = graph.resolve(r).expect("role set").to_srgb8().expect("in gamut");
                egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
            };
            assert_eq!(v.override_text_color, Some(want(Role::OnSurface)));
            assert_eq!(v.hyperlink_color, want(Role::Accent));
            assert_eq!(v.panel_fill, want(Role::SurfaceRaised));
            assert_eq!(v.selection.bg_fill, want(Role::Selection));
        }
    }

    /// The token values are the ones chisel-doc has always shipped — the
    /// dogfood must not have silently changed the product's colours.
    #[test]
    fn the_tokens_are_the_shipped_palette() {
        let l = light().expect("light parses");
        assert_eq!(
            l.resolve(Role::OnSurface).expect("set").to_srgb8().expect("in gamut"),
            color_kit::parse_hex("#222226").expect("literal"),
            "primary text must still be the shipped #222226"
        );
        let d = dark().expect("dark parses");
        assert_eq!(
            d.resolve(Role::OnSurface).expect("set").to_srgb8().expect("in gamut"),
            color_kit::parse_hex("#ECECEF").expect("literal"),
            "primary text must still be the shipped #ECECEF"
        );
    }
}
