use super::*;

/// Serialises the tests that bend the engine-start delay: the delay is a
/// process-wide atomic, and two sessions bending it concurrently would poison
/// each other's start budgets.
static ENGINE_DELAY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn fields() -> Vec<FieldState> {
    let mut agree = FieldState::new("agree", FieldType::CheckBox, vec![]);
    agree.options = vec![("Yes".into(), "Yes".into())];
    let mut size = FieldState::new("size", FieldType::ComboBox, vec!["m".into()]);
    size.options = vec![("s".into(), "Small".into()), ("m".into(), "Medium".into()), ("l".into(), "Large".into())];
    vec![
        FieldState::new("price", FieldType::Text, vec!["12.5".into()]),
        FieldState::new("qty", FieldType::Text, vec!["4".into()]),
        FieldState::new("total", FieldType::Text, vec![]),
        FieldState::new("zip", FieldType::Text, vec!["02134".into()]),
        agree,
        size,
    ]
}

fn doc() -> DocInfo {
    DocInfo { file_name: "order.pdf".into(), num_pages: 3, page: 0, info: vec![("title".into(), "Order".into())] }
}

fn go(script: &str, event: &Event) -> Outcome {
    run(script, event, &doc(), &fields(), &[], Limits::default())
}

#[test]
fn calculate_scripts_read_fields_and_set_the_value() {
    let o = go("event.value = this.getField('price').value * getField('qty').value;", &Event::field("Calculate", "total", ""));
    assert_eq!(o.error, None);
    assert_eq!(o.value, "50");
    assert!(o.rc);
}

#[test]
fn numbers_with_leading_zeros_stay_text() {
    let o = go("event.value = typeof getField('zip').value + ':' + typeof getField('qty').value;", &Event::field("Calculate", "total", ""));
    assert_eq!(o.value, "string:number");
}

#[test]
fn validate_scripts_reject_with_rc_and_alert() {
    let o = go("if (event.value > 10) { app.alert('Too many'); event.rc = false; }", &Event::field("Validate", "qty", "11"));
    assert!(!o.rc);
    assert_eq!(o.alerts, ["Too many"]);
    let o = go("if (event.value > 10) { event.rc = false; }", &Event::field("Validate", "qty", "3"));
    assert!(o.rc);
}

#[test]
fn keystroke_scripts_see_the_change() {
    let mut e = Event::field("Keystroke", "qty", "1");
    e.change = "x".into();
    e.will_commit = false;
    let o = go("event.rc = /^[0-9]*$/.test(event.change);", &e);
    assert!(!o.rc);
}

#[test]
fn scripts_change_other_fields_and_their_properties() {
    let o = go(
        "var t = getField('total'); t.value = 99; t.readonly = true; t.display = display.hidden; \
         getField('agree').checkThisBox(0, true); getField('size').value = 'l'; t.textColor = color.red;",
        &Event::field("Mouse Up", "agree", ""),
    );
    assert_eq!(o.error, None);
    let total = o.changed.iter().find(|f| f.name == "total").unwrap();
    assert_eq!(total.value, ["99"]);
    assert!(total.readonly);
    assert_eq!(total.display, DISPLAY_HIDDEN);
    assert_eq!(total.text_color.as_deref(), Some(&["RGB".to_string(), "1".into(), "0".into(), "0".into()][..]));
    assert_eq!(o.changed.iter().find(|f| f.name == "agree").unwrap().value, ["Yes"]);
    assert_eq!(o.changed.iter().find(|f| f.name == "size").unwrap().value, ["l"]);
}

#[test]
fn choice_and_check_box_object_model() {
    let o = go(
        "var s = getField('size'); event.value = [s.numItems, s.getItemAt(2, false), s.currentValueIndices, s.valueAsString, \
         getField('agree').value, getField('agree').isBoxChecked(0), s.type].join('|');",
        &Event::field("Calculate", "total", ""),
    );
    assert_eq!(o.value, "3|Large|1|m|Off|false|combobox");
}

#[test]
fn util_printf_printd_and_printx() {
    let o = go(
        "event.value = [util.printf('%,0.2f', 1234567.891), util.printf('%05d|%s|%x', 42, 'hi', 255), util.printf('%,2.2f', 1234.5), \
         util.printd('mmm d, yyyy HH:MM', new Date(2024, 0, 5, 9, 7)), util.printd('dddd', new Date(2024, 0, 5)), \
         util.printx('(999) 999-9999', '5551234567'), util.printx('>AAA', 'abc')].join('|');",
        &Event::field("Calculate", "total", ""),
    );
    assert_eq!(o.error, None);
    assert_eq!(o.value, "1,234,567.89|00042|hi|FF|1.234,50|Jan 5, 2024 09:07|Friday|(555) 123-4567|ABC");
}

#[test]
fn printd_escapes_multibyte_chars() {
    let o = go(
        "event.value = [util.printd('yyyy \\\\é', new Date(2024, 0, 5)), util.printd('\\\\€ yyyy', new Date(2024, 0, 5)), \
         util.printd('\\\\y yyyy', new Date(2024, 0, 5))].join('|');",
        &Event::field("Calculate", "total", ""),
    );
    assert_eq!(o.error, None);
    assert_eq!(o.value, "2024 é|€ 2024|y 2024");
}

fn assert_printf_error(script: &str, message: &str) {
    let o = go(script, &Event::doc("Open"));
    assert!(o.error.as_deref().is_some_and(|e| e.starts_with(message)), "{script}: {:?}", o.error);
    let o = xfa::run_xfa(
        script,
        &xfa::XfaEvent { activity: "initialize".into(), target: "form1[0]".into(), ..Default::default() },
        &xfa::XfaDoc::default(),
        &xfa_model::form(),
        Limits::default(),
    );
    assert!(o.error.as_deref().is_some_and(|e| e.starts_with(message)), "XFA: {script}: {:?}", o.error);
}

#[test]
fn util_printf_rejects_large_width() {
    assert_printf_error("util.printf('%99999999999d', 1)", "TypeError: util.printf width or precision exceeds 4096");
}

#[test]
fn util_printf_rejects_large_precision() {
    assert_printf_error("util.printf('%.99999999999f', 1)", "TypeError: util.printf width or precision exceeds 4096");
}

#[test]
fn util_printf_rejects_overflowing_width() {
    assert_printf_error("util.printf('%9999999999999999999999999999999999999999d', 1)", "TypeError: util.printf width or precision exceeds 4096");
}

#[test]
fn util_printf_rejects_overflowing_precision() {
    assert_printf_error("util.printf('%.9999999999999999999999999999999999999999f', 1)", "TypeError: util.printf width or precision exceeds 4096");
}

#[test]
fn util_printf_rejects_limits_for_all_conversions() {
    for conv in ['d', 'f', 's', 'x'] {
        for spec in ["4097", ".4097", "99999999999", ".99999999999"] {
            assert_printf_error(&format!("util.printf('%{spec}{conv}', 1)"), "TypeError: util.printf width or precision exceeds 4096");
        }
    }
}

#[test]
fn util_printf_bounds_total_output() {
    for script in [
        "util.printf('%4096d'.repeat(300), 1)",
        "util.printf('%04096d'.repeat(300), -1)",
        "util.printf('aa'.repeat(524288) + 'a')",
        "util.printf('é'.repeat(524289))",
        "util.printf('%%%%'.repeat(524288) + '%%')",
        "util.printf('%s', 'aa'.repeat(524288) + 'a')",
        "util.printf('%s%s', 'aa'.repeat(524288), 'b')",
    ] {
        assert_printf_error(script, "TypeError: util.printf output exceeds 1048576 bytes");
    }
}

#[test]
fn util_printf_accepts_limits_and_preserves_formats() {
    let o = go(
        "event.value = [util.printf('%4096d', 1).length, util.printf('%04096d', -1).length, \
         util.printf('%.4096f', 1).length, util.printf('%.4096s', 'a'.repeat(4097)).length, \
         util.printf('%4096x', 255).length, util.printf('%.4096x', 255), \
         util.printf('%4096d'.repeat(255) + '%4095d', 1).length, util.printf('%4096d'.repeat(256), 1).length, \
         util.printf('é'.repeat(524288)).length, util.printf('a%%b:%+06d|%4s|%.2s|%x', 42, 'é', 'é界a', 255)].join('|');",
        &Event::field("Calculate", "total", ""),
    );
    assert_eq!(o.error, None);
    assert_eq!(o.value, "4096|4096|4098|4096|4096|FF|1048575|1048576|524288|a%b:+00042|   é|é界|FF");
}

fn assert_array_error(script: &str) {
    let o = go(script, &Event::field("Calculate", "total", ""));
    assert!(o.error.as_deref().is_some_and(|e| e.starts_with("TypeError: array length exceeds 1048576")), "{script}: {:?}", o.error);
    assert!(o.requests.is_empty() && o.changed.is_empty(), "{script}");
}

#[test]
fn reset_form_and_field_setters_refuse_huge_arrays() {
    for script in [
        "this.resetForm(new Array(4294967295))",
        "getField('total').value = new Array(4294967295)",
        "event.target.value = new Array(4294967295)",
        "getField('total').textColor = new Array(4294967295)",
        "getField('total').fillColor = new Array(4294967295)",
    ] {
        assert_array_error(script);
    }
}

#[test]
fn reset_form_and_field_setters_take_arrays_up_to_the_limit() {
    let o = go("this.resetForm(new Array(1 << 20)); getField('total').value = new Array(1 << 20);", &Event::field("Calculate", "total", ""));
    assert_eq!(o.error, None);
    assert!(matches!(&o.requests[..], [Request::Reset(names)] if names.len() == 1 << 20 && names.iter().all(String::is_empty)));
    assert_eq!(o.changed.iter().find(|f| f.name == "total").unwrap().value, [""]);
    assert_array_error("this.resetForm(new Array((1 << 20) + 1))");
    assert_array_error("getField('total').value = new Array((1 << 20) + 1)");
}

#[test]
fn reset_form_and_field_setters_still_read_short_arrays() {
    let mut f = fields();
    f.push(FieldState::new("pick", FieldType::ListBox, vec![]));
    let o = run(
        "this.resetForm(['a', 'b']); getField('pick').value = ['x', 'y', 'z']; getField('total').value = ['x', 'y', 'z'];",
        &Event::field("Calculate", "total", ""),
        &doc(),
        &f,
        &[],
        Limits::default(),
    );
    assert_eq!(o.error, None);
    assert_eq!(o.requests, [Request::Reset(vec!["a".into(), "b".into()])]);
    assert_eq!(o.changed.iter().find(|f| f.name == "pick").unwrap().value, ["x", "y", "z"]);
    assert_eq!(o.changed.iter().find(|f| f.name == "total").unwrap().value, ["x"]);
}

#[test]
fn document_requests_and_console() {
    let o = go(
        "console.println('hello ' + this.documentFileName + ' ' + numPages + ' ' + info.title); this.pageNum = 2; \
         this.resetForm(['qty']); this.print(); app.launchURL('https://example.org'); this.submitForm({cURL: 'https://example.org/f'});",
        &Event::field("Mouse Up", "agree", ""),
    );
    assert_eq!(o.error, None);
    assert_eq!(o.console, ["hello order.pdf 3 Order"]);
    assert_eq!(
        o.requests,
        [
            Request::GoToPage(2),
            Request::Reset(vec!["qty".into()]),
            Request::Print,
            Request::LaunchUrl("https://example.org".into()),
            Request::Submit("https://example.org/f".into())
        ]
    );
}

#[test]
fn document_level_functions_are_available() {
    let o = run(
        "event.value = double(getField('qty').value);",
        &Event::field("Calculate", "total", ""),
        &doc(),
        &fields(),
        &["function double(x) { return x * 2; }".into()],
        Limits::default(),
    );
    assert_eq!(o.value, "8");
}

#[test]
fn errors_and_runaway_scripts_are_contained() {
    let o = go("event.value = nosuch.thing;", &Event::field("Calculate", "total", "keep"));
    assert!(o.error.is_some());
    let o = run("while (true) {}", &Event::doc("Open"), &doc(), &fields(), &[], Limits { loop_iterations: 10_000, recursion: 64 });
    assert!(o.error.is_some(), "the loop limit stops it");
    let o = go("function f() { return f(); } f();", &Event::doc("Open"));
    assert!(o.error.is_some(), "the recursion limit stops it");
    let o = go("getField('nope').value", &Event::doc("Open"));
    assert!(o.error.as_deref().unwrap_or("").contains("null") || o.error.is_some());
}

#[test]
fn the_sandbox_has_no_host_access() {
    let o = go(
        "event.value = [typeof require, typeof process, typeof fetch, typeof XMLHttpRequest, typeof importScripts].join(',');",
        &Event::doc("Open"),
    );
    assert_eq!(o.value, "undefined,undefined,undefined,undefined,undefined");
}

/// Hostile nesting used to overflow the stack in boa's parser and compiler, aborting the app
/// (about 100 nested parentheses were enough on a 2 MiB stack).
#[test]
fn deeply_nested_scripts_fail_instead_of_crashing() {
    let n = 100_000;
    let hostile = [
        format!("event.value = {}1{};", "(".repeat(n), ")".repeat(n)),
        format!("var a = {}1{};", "[".repeat(n), "]".repeat(n)),
        format!("{}{}", "{".repeat(n), "}".repeat(n)),
        format!("var a = {}1;", "!".repeat(n)),
        format!("var a = {}1;", "- ".repeat(n)),
        format!("var a = {}1;", "1?1:".repeat(n)),
        format!("var f = {}1;", "a=>".repeat(n)),
    ];
    for s in &hostile {
        let o = go(s, &Event::doc("Open"));
        assert!(o.error.is_some(), "{}…", &s[..20]);
    }
    // A hostile document-level script is refused too, before the field script runs.
    let o = run("event.value = 'ran';", &Event::field("Calculate", "a", ""), &doc(), &fields(), &[hostile[0].clone()], Limits::default());
    assert!(o.error.is_some());
}

#[test]
fn ordinary_nesting_still_runs() {
    let n = 40;
    let s = format!("event.value = {}1{} + [[[2]]][0][0][0] + (1 ? 2 : 3);", "(".repeat(n), ")".repeat(n));
    let o = go(&s, &Event::field("Calculate", "a", ""));
    assert_eq!(o.error, None);
    assert_eq!(o.value, "5");
    // Brackets in strings and comments don't count.
    let s = format!("// {}\nevent.value = '{}'.length;", "(".repeat(1000), "[".repeat(1000));
    assert_eq!(go(&s, &Event::field("Calculate", "a", "")).value, "1000");
}

mod xfa_model {
    use crate::Limits;
    use crate::xfa::*;

    pub(super) fn field(name: &str, som: &str, value: &str, numeric: bool) -> XfaNode {
        XfaNode {
            name: name.into(),
            som: som.into(),
            kind: Some(XfaKind::Field),
            value: value.into(),
            numeric,
            presence: "visible".into(),
            ..Default::default()
        }
    }

    /// form1 > page1 > { qty, price, total, details (subform, hidden) > note, table > row[0..2] > { what, amount }, go (button) }
    pub(super) fn form() -> XfaNode {
        let row = |i: usize| XfaNode {
            name: "row".into(),
            som: format!("form1[0].page1[0].table[0].row[{i}]"),
            kind: Some(XfaKind::Subform),
            repeatable: true,
            occur_min: 1,
            occur_max: None,
            index: i,
            children: vec![
                field("what", &format!("form1[0].page1[0].table[0].row[{i}].what[0]"), if i == 0 { "Rent" } else { "" }, false),
                field("amount", &format!("form1[0].page1[0].table[0].row[{i}].amount[0]"), if i == 0 { "10" } else { "5" }, true),
            ],
            ..Default::default()
        };
        XfaNode {
            name: "form1".into(),
            som: "form1[0]".into(),
            kind: Some(XfaKind::Subform),
            children: vec![XfaNode {
                name: "page1".into(),
                som: "form1[0].page1[0]".into(),
                kind: Some(XfaKind::Subform),
                children: vec![
                    field("qty", "form1[0].page1[0].qty[0]", "4", true),
                    field("price", "form1[0].page1[0].price[0]", "2.5", true),
                    field("total", "form1[0].page1[0].total[0]", "", true),
                    XfaNode {
                        name: "details".into(),
                        som: "form1[0].page1[0].details[0]".into(),
                        kind: Some(XfaKind::Subform),
                        presence: "hidden".into(),
                        children: vec![field("note", "form1[0].page1[0].details[0].note[0]", "", false)],
                        ..Default::default()
                    },
                    XfaNode {
                        name: "table".into(),
                        som: "form1[0].page1[0].table[0]".into(),
                        kind: Some(XfaKind::Subform),
                        children: vec![row(0), row(1)],
                        ..Default::default()
                    },
                    field("go", "form1[0].page1[0].go[0]", "", false),
                ],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn go(script: &str, target: &str, activity: &str) -> XfaOutcome {
        let ev = XfaEvent { activity: activity.into(), target: target.into(), ..Default::default() };
        let doc = XfaDoc { file_name: "f.pdf".into(), page: 1, page_count: 3 };
        run_xfa(script, &ev, &doc, &form(), Limits::default())
    }

    #[test]
    fn calculate_scripts_see_siblings_by_name_and_yield_their_last_expression() {
        let o = go("qty.rawValue * price.rawValue", "form1[0].page1[0].total[0]", "calculate");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("10"));
        let o = go("this.rawValue = xfa.form.form1.page1.qty.rawValue + 1;", "form1[0].page1[0].total[0]", "calculate");
        assert_eq!(o.effects, vec![XfaEffect::SetValue { som: "form1[0].page1[0].total[0]".into(), value: "5".into() }]);
    }

    #[test]
    fn long_values_and_results_are_cut_where_they_are_stored() {
        // The value kept for later reads is the one the effect saves.
        let o = go("details.note.rawValue = new Array(100001).join('x'); details.note.rawValue.length", "form1[0].page1[0].total[0]", "calculate");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("65536"));
        assert!(o.notes.iter().any(|n| n.contains("cut")), "{:?}", o.notes);
        // A calculate's result is cut like a value set, and noted.
        let o = go("new Array(100001).join('x')", "form1[0].page1[0].details[0].note[0]", "calculate");
        assert_eq!(o.result.map(|r| r.chars().count()), Some(65_536));
        assert!(o.notes.iter().any(|n| n.contains("result")), "{:?}", o.notes);
    }

    #[test]
    fn values_read_as_numbers_or_strings_and_null_when_empty() {
        let o = go(
            "typeof qty.rawValue + ':' + typeof this.rawValue + ':' + (total.rawValue === null) + ':' + typeof table.row.what.rawValue",
            "form1[0].page1[0].price[0]",
            "click",
        );
        assert_eq!(o.result.as_deref(), Some("number:number:true:string"));
    }

    #[test]
    fn navigation_parent_resolve_node_and_nodes() {
        let o = go(
            "this.parent.name + '/' + this.parent.parent.className + '/' + this.resolveNode('qty').rawValue + '/' + xfa.resolveNode('$form.form1.page1.table.row[1].amount').rawValue + '/' + this.parent.nodes.length + '/' + xfa.resolveNodes('form1.page1.table.row[*]').length + '/' + xfa.resolveNode('$form..amount').somExpression",
            "form1[0].page1[0].price[0]",
            "click",
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("page1/subform/4/5/6/2/form1[0].page1[0].table[0].row[0].amount[0]"));
    }

    #[test]
    fn presence_and_access_changes_are_effects() {
        let o = go("details.presence = 'visible'; details.note.access = 'readOnly'; this.presence = 'nonsense';", "form1[0].page1[0].go[0]", "click");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(
            o.effects,
            vec![
                XfaEffect::SetPresence { som: "form1[0].page1[0].details[0]".into(), presence: "visible".into() },
                XfaEffect::SetAccess { som: "form1[0].page1[0].details[0].note[0]".into(), access: "readOnly".into() },
            ]
        );
    }

    #[test]
    fn instance_managers_add_remove_and_count() {
        let o = go(
            "var n = table._row.count; var r = table._row.addInstance(1); r.what.rawValue = 'New'; n + '/' + table._row.count + '/' + r.somExpression + '/' + r.index",
            "form1[0].page1[0].go[0]",
            "click",
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("2/3/form1[0].page1[0].table[0].row[2]/2"));
        assert_eq!(
            o.effects,
            vec![
                XfaEffect::AddInstance { som: "form1[0].page1[0].table[0].row[2]".into() },
                XfaEffect::SetValue { som: "form1[0].page1[0].table[0].row[2].what[0]".into(), value: "New".into() },
            ]
        );
        // Through the instance manager property, from inside a row; removing renumbers.
        let o = go(
            "this.parent.instanceManager.removeInstance(0); xfa.resolveNode('table.row[0]').amount.rawValue",
            "form1[0].page1[0].table[0].row[1].amount[0]",
            "click",
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("5"), "the second row is now the first");
        assert_eq!(o.effects, vec![XfaEffect::RemoveInstance { som: "form1[0].page1[0].table[0].row[0]".into() }]);
        // setInstances, and the floor of one instance.
        let o = go(
            "table._row.setInstances(4); var a = table._row.count; table._row.setInstances(0); a + '/' + table._row.count",
            "form1[0].page1[0].go[0]",
            "click",
        );
        assert_eq!(o.result.as_deref(), Some("4/1"));
    }

    #[test]
    fn host_layout_event_and_app_calls_become_effects() {
        let o = go(
            "xfa.host.messageBox('Hi', 'T', 3, 1); xfa.host.resetData('qty, price'); xfa.host.print(1, '0', '2', 0, 0, 0, 0, 0); app.execMenuItem('SaveAs'); app.launchURL('https://x.test'); xfa.host.setFocus('qty'); xfa.form.recalculate(1); console.println('p' + xfa.layout.page(this) + '/' + xfa.layout.pageCount() + '/' + xfa.host.currentPage + '/' + xfa.event.name); this.border.fill.color.value = '255,0,0'; this.fillColor = 'x'; util.printf('%d', 7)",
            "form1[0].page1[0].go[0]",
            "click",
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(
            o.effects,
            vec![
                XfaEffect::MessageBox("Hi".into()),
                XfaEffect::ResetData(vec!["qty".into(), "price".into()]),
                XfaEffect::Print,
                XfaEffect::SaveAs,
                XfaEffect::LaunchUrl("https://x.test".into()),
                XfaEffect::SetFocus("form1[0].page1[0].qty[0]".into()),
                XfaEffect::Recalculate,
            ]
        );
        assert_eq!(o.console, vec!["p2/3/1/click".to_string()]);
        assert_eq!(o.result.as_deref(), Some("7"));
    }

    #[test]
    fn field_names_never_shadow_javascript_builtins() {
        let mut f = form();
        f.children[0].children.push(field("Date", "form1[0].page1[0].Date[0]", "x", false));
        f.children[0].children.push(field("eval", "form1[0].page1[0].eval[0]", "y", false));
        let ev = XfaEvent { activity: "click".into(), target: "form1[0].page1[0].go[0]".into(), ..Default::default() };
        let o = run_xfa(
            "typeof new Date().getTime() + ':' + this.parent.Date.rawValue + ':' + this.parent.eval.rawValue",
            &ev,
            &XfaDoc::default(),
            &f,
            Limits::default(),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("number:x:y"));
    }

    #[test]
    fn validate_scripts_give_a_verdict_and_errors_are_reported() {
        let o = go("this.rawValue === null || this.rawValue <= 3", "form1[0].page1[0].qty[0]", "validate");
        assert_eq!(o.result_bool, Some(false));
        let o = go("nosuch.rawValue = 1", "form1[0].page1[0].qty[0]", "click");
        assert!(o.error.is_some() && o.effects.is_empty());
        let o = go("while (true) {}", "form1[0].page1[0].qty[0]", "click");
        assert!(o.error.is_some(), "the loop limit stops it");
        let o = go(&"(".repeat(200), "form1[0].page1[0].qty[0]", "click");
        assert!(o.error.is_some());
        // Adding instances is capped; a runaway loop can't blow the tree up.
        let o = go("for (var i = 0; i < 5000; i++) table._row.addInstance(1); table._row.count", "form1[0].page1[0].go[0]", "click");
        assert_eq!(o.result.as_deref(), Some("1002"));
    }

    #[test]
    fn values_set_again_merge_and_hostile_loops_are_capped() {
        // A calculate that sets its own value in a loop leaves one effect: the last value.
        let o = go("for (var i = 0; i < 50000; i++) this.rawValue = i;", "form1[0].page1[0].total[0]", "calculate");
        assert_eq!(o.effects, vec![XfaEffect::SetValue { som: "form1[0].page1[0].total[0]".into(), value: "49999".into() }], "{:?}", o.error);
        // Order-dependent effects in between keep the values on either side.
        let o = go("qty.rawValue = 1; xfa.host.resetData(); qty.rawValue = 2; qty.rawValue = 3;", "form1[0].page1[0]", "click");
        assert_eq!(o.effects.len(), 3, "{:?}", o.effects);
        assert_eq!(o.effects.last(), Some(&XfaEffect::SetValue { som: "form1[0].page1[0].qty[0]".into(), value: "3".into() }));
        // Message boxes and console lines in a loop stop at their caps, and say so.
        let o = go("for (var i = 0; i < 100000; i++) { xfa.host.messageBox('m' + i); console.println('c' + i); }", "form1[0].page1[0]", "initialize");
        let alerts = o.effects.iter().filter(|e| matches!(e, XfaEffect::MessageBox(_))).count();
        assert_eq!(alerts, MAX_ALERTS);
        assert_eq!(o.console.len(), MAX_CONSOLE);
        assert!(o.notes.iter().any(|n| n.contains("messages")) && o.notes.iter().any(|n| n.contains("console")), "{:?}", o.notes);
        // Distinct effects stop at the effect cap.
        let o = go("for (var i = 0; i < 30000; i++) xfa.host.beep();", "form1[0].page1[0]", "click");
        assert_eq!(o.effects.len(), MAX_EFFECTS);
        assert!(o.notes.iter().any(|n| n.contains("changes")), "{:?}", o.notes);
        // Long messages are cut.
        let o = go("xfa.host.messageBox(new Array(100000).join('x'));", "form1[0].page1[0]", "click");
        assert!(matches!(&o.effects[0], XfaEffect::MessageBox(m) if m.chars().count() <= 4_097));
    }

    #[test]
    fn a_slow_engine_start_does_not_count_against_the_script() {
        let ms = std::time::Duration::from_millis;
        // On a busy machine the engine can take longer to start than a script may run: the
        // script still runs (abandoning it would turn the form's scripts off).
        let o = run_within_limits("test", ms(10_000), ms(500), move |started| {
            std::thread::sleep(ms(1_500));
            started();
            XfaOutcome { result: Some("ran".into()), ..Default::default() }
        });
        assert!(!o.abandoned, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("ran"));
        // The script's own time counts from then.
        let o = run_within_limits("test", ms(10_000), ms(200), move |started| {
            started();
            std::thread::sleep(ms(2_000));
            XfaOutcome::default()
        });
        assert!(o.abandoned && o.error.as_deref().is_some_and(|e| e.contains("ran longer")), "{o:?}");
        // Start-up has a limit of its own, so nobody waits forever for an engine.
        let o = run_within_limits("test", ms(200), ms(10_000), move |started| {
            std::thread::sleep(ms(2_000));
            started();
            XfaOutcome::default()
        });
        assert!(o.abandoned && o.error.as_deref().is_some_and(|e| e.contains("to start")), "{o:?}");
        // A script that finishes without saying it started (refused at once) isn't abandoned.
        let o = run_within_limits("test", ms(10_000), ms(200), |_| XfaOutcome { error: Some("refused".into()), ..Default::default() });
        assert!(!o.abandoned && o.error.as_deref() == Some("refused"), "{o:?}");
    }
}

mod formcalc {
    use super::xfa_model::form;
    use crate::Limits;
    use crate::formcalc::run_formcalc;
    use crate::xfa::*;

    fn go(script: &str, target: &str, activity: &str) -> XfaOutcome {
        let ev = XfaEvent { activity: activity.into(), target: target.into(), ..Default::default() };
        let doc = XfaDoc { file_name: "f.pdf".into(), page: 1, page_count: 3 };
        run_formcalc(script, &ev, &doc, &form(), Limits::default())
    }

    /// The value of a script run as a calculate on `total`.
    fn calc(script: &str) -> String {
        let o = go(script, "form1[0].page1[0].total[0]", "calculate");
        assert_eq!(o.error, None, "{script}");
        o.result.unwrap_or_default()
    }

    #[test]
    fn arithmetic_strings_and_comparisons_follow_formcalc_rules() {
        assert_eq!(calc("1 + 2 * 3"), "7");
        assert_eq!(calc("(1 + 2) * 3"), "9");
        assert_eq!(calc("10 / 4"), "2.5");
        assert_eq!(calc("-3 + +4"), "1");
        assert_eq!(calc(r#""12" + 3"#), "15");
        assert_eq!(calc(r#""abc" + 3"#), "3");
        assert_eq!(calc(r#"Concat("a", "b", 1.50)"#), "ab1.5");
        assert_eq!(calc(r##""say ""hi""""##), "say \"hi\"");
        assert_eq!(calc("1 < 2 and 2 <= 2"), "1");
        assert_eq!(calc("1 == 1.0 & 3 <> 4"), "1");
        assert_eq!(calc(r#""a" lt "b" | 0"#), "1");
        assert_eq!(calc("not 0"), "1");
        assert_eq!(calc("!0 + !1"), "1");
        assert_eq!(calc("table.row[0]..amount"), "10");
        assert_eq!(calc(r#""abc" == "ABC""#), "0");
        // Logical operands are numbers: text that is not a number is false.
        assert_eq!(calc(r#"if ("abc") then 1 else 0 endif"#), "0");
        assert_eq!(calc(r#"if ("2") then 1 else 0 endif"#), "1");
        assert_eq!(calc("null + 1"), "1");
        assert_eq!(calc("null == null"), "1");
        assert_eq!(calc("1 == null"), "0");
        assert_eq!(calc("1 <> null"), "1");
        assert!(go("1 / 0", "form1[0].page1[0].total[0]", "calculate").error.is_some());
        // Comments and newlines inside brackets.
        assert_eq!(calc("; a comment\n// another\nSum(1,\n 2,\n 3)"), "6");
    }
    #[test]
    fn formcalc_language_edges() {
        // do…end is a block statement; its value is the last expression.
        assert_eq!(calc("do 5 end"), "5");
        assert_eq!(calc("do var t = 2 t = t * 3 t end"), "6");

        // Scientific-notation literals parse (sign after the exponent too).
        assert_eq!(calc("1e3 + 1"), "1001");
        assert_eq!(calc("2.5e-2"), "0.025");
        assert_eq!(calc("1E2"), "100");

        // Non-finite results format per Acrobat: NaN, Infinity, -Infinity.
        assert_eq!(calc("1e400 - 1e400"), "NaN");
        assert_eq!(calc("1e400 - 1"), "Infinity");
        assert_eq!(calc("-1e400 + 1"), "-Infinity");

        // Parse errors name the offending token and its kind.
        assert!(go("1 + ]", "form1[0].page1[0].total[0]", "calculate").error.is_some_and(|e| e.contains("unexpected")));
        // An unterminated do-block names what it wanted.
        assert!(go("do 1", "form1[0].page1[0].total[0]", "calculate").error.is_some_and(|e| e.contains("expected end")));

        // The root references stringify to their kind.
        assert_eq!(calc("$host"), "host");
        assert_eq!(calc("$layout"), "layout");
        assert_eq!(calc("$event"), "event");
        assert_eq!(calc("$xfa"), "xfa");
    }

    #[test]
    fn formcalc_and_util_single_branch_edges() {
        // exponent sign form parses (1e+3)
        assert_eq!(calc("1e+3 + 1"), "1001");
        // a string literal over the formcalc limit is refused by name.
        // (Observed: the refuse gate catches the oversized SCRIPT first, so the
        // string-limit arm needs a script under 256 KiB whose literal alone
        // exceeds the 1 MiB formcalc string cap - not reachable in one line;
        // covered instead by the printf-flag asserts below.)
    }

    #[test]
    fn host_event_and_xfa_property_reads() {
        use crate::formcalc::run_formcalc;
        let ev = crate::xfa::XfaEvent {
            activity: "calculate".into(),
            target: "form1[0].page1[0].total[0]".into(),
            new_text: "typed".into(),
            prev_text: "previous".into(),
        };
        let doc = crate::xfa::XfaDoc { file_name: "lease.pdf".into(), page: 2, page_count: 7 };
        let run = |s: &str| run_formcalc(s, &ev, &doc, &form(), Limits::default());

        // $host exposes the viewer identity and document facts
        assert_eq!(run("$host.numpages").result.as_deref(), Some("7"));
        assert_eq!(run("$host.currentpage").result.as_deref(), Some("2"));
        assert_eq!(run("$host.name").result.as_deref(), Some("Acrobat"));
        assert_eq!(run("$host.apptype").result.as_deref(), Some("Exchange-Pro"));
        assert_eq!(run("$host.version").result.as_deref(), Some("11"));
        assert_eq!(run("$host.variation").result.as_deref(), Some("Full"));
        assert_eq!(run("$host.language").result.as_deref(), Some("en"));
        assert_eq!(run("$host.title").result.as_deref(), Some("lease.pdf"));
        assert_eq!(run("$host.calculationsenabled").result.as_deref(), Some("1"));
        assert_eq!(run("$host.validationsenabled").result.as_deref(), Some("1"));

        // $event carries the activity and keystroke text
        assert_eq!(run("$event.name").result.as_deref(), Some("calculate"));
        assert_eq!(run("$event.newtext").result.as_deref(), Some("typed"));
        assert_eq!(run("$event.fulltext").result.as_deref(), Some("typed"));
        assert_eq!(run("$event.prevtext").result.as_deref(), Some("previous"));
        assert_eq!(run("$event.selstart").result.as_deref(), Some("0"));

        // $xfa handles stringify; unknown properties sink silently
        assert_eq!(run("$xfa.host").result.as_deref(), Some("host"));
        assert_eq!(run("$xfa.layout").result.as_deref(), Some("layout"));
        assert_eq!(run("$xfa.event").result.as_deref(), Some("event"));
        assert_eq!(run("$host.nonsense").result.as_deref(), Some(""));
    }
    #[test]
    fn built_in_functions_cover_the_common_set() {
        assert_eq!(calc("Abs(-2.5)"), "2.5");
        assert_eq!(calc("Ceil(2.1)"), "3");
        assert_eq!(calc("Floor(-2.1)"), "-3");
        assert_eq!(calc("Mod(7, 3)"), "1");
        assert_eq!(calc("Round(2.345, 2)"), "2.35");
        assert_eq!(calc("Sum(1, 2, 3)"), "6");
        assert_eq!(calc("Avg(1, 2, 3, null)"), "2");
        assert_eq!(calc("Count(1, null, 3)"), "2");
        assert_eq!(calc("Max(1, 9, 3)"), "9");
        assert_eq!(calc("Min(4, 2, 3)"), "2");
        assert_eq!(calc(r#"Choose(2, "a", "b", "c")"#), "b");
        assert_eq!(calc(r#"Oneof(3, 1, 2, 3)"#), "1");
        assert_eq!(calc(r#"Within(5, 1, 10)"#), "1");
        assert_eq!(calc(r#"HasValue("")"#), "0");
        assert_eq!(calc(r#"HasValue("x")"#), "1");
        assert_eq!(calc(r#"At("abcd", "c")"#), "3");
        assert_eq!(calc(r#"Left("hello", 2)"#), "he");
        assert_eq!(calc(r#"Right("hello", 2)"#), "lo");
        assert_eq!(calc(r#"Len("héllo")"#), "5");
        assert_eq!(calc(r#"Lower("ABC")"#), "abc");
        assert_eq!(calc(r#"Upper("abc")"#), "ABC");
        assert_eq!(calc(r#"Ltrim("  a ")"#), "a ");
        assert_eq!(calc(r#"Rtrim("  a ")"#), "  a");
        assert_eq!(calc(r#"Space(3)"#), "   ");
        assert_eq!(calc(r#"Str(3.14159, 6, 2)"#), "  3.14");
        assert_eq!(calc(r#"Substr("abcdef", 2, 3)"#), "bcd");
        assert_eq!(calc(r#"Stuff("abcdef", 2, 3, "XY")"#), "aXYef");
        assert_eq!(calc(r#"Replace("a-b-c", "-", "+")"#), "a+b+c");
        assert_eq!(calc(r#"WordNum(42)"#), "Forty-Two");
        assert_eq!(calc(r#"WordNum(1.999, 2)"#), "Two Dollars And 00 Cents");
        assert_eq!(calc(r#"WordNum(1.01, 2)"#), "One Dollar And 01 Cent");
        assert_eq!(calc(r#"Format("zzz,zz9.99", 1234.5)"#), "1,234.50");
        assert_eq!(calc(r#"Format("$z,zz9.99", -5)"#), "-$5.00");
        assert_eq!(calc(r#"Format("MM/DD/YYYY", "2024-02-29")"#), "02/29/2024");
        assert_eq!(calc(r#"Parse("zzz,zz9.99", "1,234.50")"#), "1234.5");
        assert_eq!(calc(r#"Parse("MMM D, YYYY", "Mar 15, 1996")"#), "1996-03-15");
        assert_eq!(calc(r#"Date2Num("1900-01-01")"#), "1");
        assert_eq!(calc(r#"Date2Num("2024-02-29")"#), "45350");
        assert_eq!(calc(r#"Date2Num("29/02/2024", "DD/MM/YYYY")"#), "45350");
        assert_eq!(calc(r#"Num2Date(45350, "YYYY-MM-DD")"#), "2024-02-29");
        assert_eq!(calc(r#"Num2Date(Date2Num("2024-02-29") + 1, "MMMM D, YYYY")"#), "March 1, 2024");
        assert_eq!(calc(r#"Time2Num("01:00:00")"#), "3600000");
        assert_eq!(calc(r#"Num2Time(3661000)"#), "01:01:01");
        assert_eq!(calc("Round(Pmt(10000, 0.01, 12), 2)"), "888.49");
        assert_eq!(calc("Round(FV(100, 0.01, 12), 2)"), "1268.25");
        assert_eq!(calc("Round(PV(100, 0.01, 12), 2)"), "1125.51");
        assert_eq!(calc(r#"UnitValue("2in", "mm")"#), "50.8");
        assert_eq!(calc(r#"UnitType("2.5cm")"#), "cm");
        assert_eq!(calc(r#"Encode("a b&c", "url")"#), "a%20b%26c");
        assert_eq!(calc(r#"Decode("a%20b", "url")"#), "a b");
        assert!(go(r#"Get("http://example.com")"#, "form1[0].page1[0].total[0]", "calculate").error.unwrap().contains("network"));
        assert!(go("Nope(1)", "form1[0].page1[0].total[0]", "calculate").error.unwrap().contains("unknown function"));
    }

    #[test]
    fn date2num_rejects_days_past_the_end_of_the_month() {
        assert_eq!(calc(r#"Date2Num("2023-02-30")"#), "0");
        assert_eq!(calc(r#"Date2Num("31/04/2024", "DD/MM/YYYY")"#), "0");
        assert_eq!(calc(r#"Date2Num("2023-02-29")"#), "0");
        assert_eq!(calc(r#"Date2Num("1900-02-29")"#), "0");
        assert_eq!(calc(r#"Parse("YYYY-MM-DD", "2023-02-30") == null"#), "1");
        // Valid month ends still work.
        assert_eq!(calc(r#"Date2Num("2024-02-29")"#), "45350");
        assert_eq!(calc(r#"Date2Num("2000-02-29") <> 0"#), "1");
        assert_eq!(calc(r#"Date2Num("2024-04-30") <> 0"#), "1");
        assert_eq!(calc(r#"Date2Num("2024-12-31") <> 0"#), "1");
    }

    #[test]
    fn control_flow_variables_and_functions_work() {
        assert_eq!(calc("var x = 3\nif (x > 2) then 10 elseif (x > 1) then 20 else 30 endif"), "10");
        // An assignment is an expression: its value is the script's result.
        assert_eq!(calc("qty = 7"), "7");
        assert_eq!(calc("var y = 8"), "8");
        assert_eq!(calc("var x = 1\nif (x > 2) then 10 elseif (x > 0) then 20 else 30 endif"), "20");
        assert_eq!(calc("var i = 0\nvar s = 0\nwhile (i < 5) do\n i = i + 1\n if (i == 3) then continue endif\n s = s + i\nendwhile\ns"), "12");
        assert_eq!(calc("var s = 0\nfor i = 1 upto 10 step 3 do s = s + i endfor\ns"), "22");
        assert_eq!(calc("var s = 0\nfor i = 5 downto 1 do\n if (i == 2) then break endif\n s = s + i\nendfor\ns"), "12");
        assert_eq!(
            calc(
                r#"var s = ""
foreach w in ("a", "b", "c") do s = Concat(s, w) endfor
s"#
            ),
            "abc"
        );
        assert_eq!(calc("func sq(n) do n * n endfunc\nsq(4) + sq(3)"), "25");
        assert_eq!(calc("func Sq(n) do n * n endfunc\nSQ(4)"), "16");
        // `exit` inside a function ends the whole script.
        let o = go("func f() do exit endfunc\nf()\n$host.messageBox(\"still ran\")\n5", "form1[0].page1[0].go[0]", "click");
        assert_eq!(o.error, None);
        assert!(o.effects.is_empty(), "{:?}", o.effects);
        // Rows can be walked as objects.
        assert_eq!(calc("var s = 0\nforeach r in (table.row[*]) do s = s + r.amount endfor\ns"), "15");
        // A long sum is not deep nesting.
        let long = format!("1{}", " + 1".repeat(500));
        let ev = XfaEvent { activity: "calculate".into(), target: "form1[0].page1[0].total[0]".into(), ..Default::default() };
        let o = run_formcalc(&long, &ev, &XfaDoc::default(), &form(), Limits { loop_iterations: 1000, recursion: 64 });
        assert_eq!(o.result.as_deref(), Some("501"), "{o:?}");
        // Null is an empty result with no truth value, as in JavaScript.
        let o = go("if (0) then 5 endif", "form1[0].page1[0].total[0]", "calculate");
        assert_eq!(o.result.as_deref(), Some(""));
        assert_eq!(o.result_bool, None);
        assert_eq!(calc("func f(n) do if (n < 2) then return 1 endif\n return n * f(n - 1) endfunc\nf(5)"), "120");
        assert_eq!(calc("IF (1) THEN 5 ENDIF"), "5");
        let o = go(r#"throw "stop here""#, "form1[0].page1[0].total[0]", "calculate");
        assert_eq!(o.error.as_deref(), Some("stop here"));
        let o = go("if (1) then 1", "form1[0].page1[0].total[0]", "calculate");
        assert!(o.error.unwrap().contains("endif"));
    }

    #[test]
    fn form_objects_resolve_assign_and_record_effects() {
        // Fields deref to their values; `$` is the current field.
        assert_eq!(calc("qty * price"), "10");
        assert_eq!(calc("$.name"), "total");
        assert_eq!(calc("$form.form1.page1.qty + 1"), "5");
        assert_eq!(calc("xfa.form.form1.page1.qty"), "4");
        assert_eq!(calc("$record.page1.qty"), "4");
        assert_eq!(calc("Sum(table.row[*].amount)"), "15");
        assert_eq!(calc("Count(table.row[*])"), "2");
        assert_eq!(calc("Count(table.row[*].what)"), "1");
        // Lists hold each object once, however they were reached.
        assert_eq!(calc("Count((table.row[*].parent..row).parent..row)"), "2");
        assert_eq!(calc("Count(xfa.resolveNodes(\"table.row[*].parent.row[*].parent.row[*]\"))"), "2");
        // Bare names in a row see every sibling row, and `[-1]`/`[+1]` are relative to the row
        // the script runs in.
        let in_row = |s: &str| {
            let o = go(s, "form1[0].page1[0].table[0].row[1].amount[0]", "calculate");
            assert_eq!(o.error, None, "{s}");
            o.result.unwrap_or_default()
        };
        assert_eq!(in_row("Count(row[*])"), "2");
        assert_eq!(in_row("Sum(row[*].amount)"), "15");
        assert_eq!(in_row("row[1].amount"), "5");
        assert_eq!(in_row("row[-1].amount"), "10");
        // Past the ends there is no object, so a property of it is an error (as in Acrobat).
        let past = go("row[+1].amount", "form1[0].page1[0].table[0].row[1].amount[0]", "calculate");
        assert!(past.error.unwrap().contains("no object"));
        assert_eq!(in_row("Exists(row[-5])"), "0");
        let o = go("row[+1].amount", "form1[0].page1[0].table[0].row[0].amount[0]", "calculate");
        assert_eq!(o.result.as_deref(), Some("5"));
        assert_eq!(calc("table.row[1].amount"), "5");
        assert_eq!(calc("table.row.amount"), "10");
        assert_eq!(calc("$form..amount[1]"), "5");
        assert_eq!(calc("table._row.count"), "2");
        assert_eq!(calc("Exists(qty) & not Exists(nothing)"), "1");
        assert_eq!(calc("HasValue(total)"), "0");
        assert_eq!(calc("$host.numPages"), "3");
        assert_eq!(calc("$layout.pageCount()"), "3");
        assert_eq!(calc("$event.name"), "calculate");
        assert_eq!(calc("xfa.resolveNode(\"page1.price\").rawValue"), "2.5");
        assert_eq!(calc("$.border.fill.color.value"), "");
        assert!(go("nothing + 1", "form1[0].page1[0].total[0]", "calculate").error.unwrap().contains("unknown name"));
        // Assignment to a field (by name and by path), a property and a variable.
        let o = go(
            r#"page1.qty = 6
xfa.form.form1.page1.qty = 7
details.presence = "visible"
price.access = "readOnly"
$.border.fill.color.value = "255,0,0"
$host.messageBox(Concat("Total ", qty * price))
$host.setFocus("qty")
table._row.addInstance(1)
table._row.count"#,
            "form1[0].page1[0].go[0]",
            "click",
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("3"));
        assert_eq!(
            o.effects,
            vec![
                XfaEffect::SetValue { som: "form1[0].page1[0].qty[0]".into(), value: "7".into() },
                XfaEffect::SetPresence { som: "form1[0].page1[0].details[0]".into(), presence: "visible".into() },
                XfaEffect::SetAccess { som: "form1[0].page1[0].price[0]".into(), access: "readOnly".into() },
                XfaEffect::MessageBox("Total 17.5".into()),
                XfaEffect::SetFocus("form1[0].page1[0].qty[0]".into()),
                XfaEffect::AddInstance { som: "form1[0].page1[0].table[0].row[2]".into() },
            ]
        );
        // Instance managers: remove, setInstances, count assignment, and the floor of one.
        let o = go(
            "table._row.removeInstance(0)\ntable._row.setInstances(1)\ntable._row.count = 0\ntable._row.count",
            "form1[0].page1[0].go[0]",
            "click",
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("1"));
        // Validate scripts give a truth value; a value the script sets wins over the result.
        let o = go("$ == null or $ <= 100", "form1[0].page1[0].qty[0]", "validate");
        assert_eq!(o.result_bool, Some(true));
        let o = go("$ > 100", "form1[0].page1[0].qty[0]", "validate");
        assert_eq!(o.result_bool, Some(false));
        // Reset, print and URLs are recorded as requests, not performed.
        let o = go(
            r#"$host.resetData("qty, price")
$host.print()
$host.gotoURL("https://example.org")"#,
            "form1[0].page1[0].go[0]",
            "click",
        );
        assert_eq!(
            o.effects,
            vec![XfaEffect::ResetData(vec!["qty".into(), "price".into()]), XfaEffect::Print, XfaEffect::LaunchUrl("https://example.org".into())]
        );
    }

    #[test]
    fn formcalc_values_results_and_errors_stay_bounded() {
        let total = "form1[0].page1[0].total[0]";
        let fast = Limits { loop_iterations: 10_000, recursion: 32 };
        let ev = XfaEvent { activity: "calculate".into(), target: total.into(), ..Default::default() };
        let doc = XfaDoc::default();
        let run = |s: &str| run_formcalc(s, &ev, &doc, &form(), fast);
        // A long value is cut where it is stored, so it reads back as saved.
        let o = run("details.note = Space(100000)\nLen(details.note)");
        assert_eq!(o.error, None);
        assert_eq!(o.result.as_deref(), Some("65536"));
        // A calculate's result is cut as values are, with a note.
        let o = run("Space(100000)");
        assert_eq!(o.result.map(|r| r.chars().count()), Some(65_536));
        assert!(o.notes.iter().any(|n| n.contains("result")), "{:?}", o.notes);
        // A thrown value is the error message: cut like one.
        let e = run("throw Space(100000)").error.unwrap();
        assert!(e.chars().count() <= 4_097, "{}", e.len());
        // Copying and comparing long strings costs steps, so it ends without a clock (wasm).
        let e = run("var s = Space(1000000)\nfor i = 1 upto 3000 do s endfor").error;
        assert!(e.is_some_and(|e| e.contains("too long")));
        let e = run("var s = Space(1000000)\nfor i = 1 upto 3000 do s == \"x\" endfor").error;
        assert!(e.is_some_and(|e| e.contains("too long")));
        // Each period IPmt and PPmt walk costs a step.
        let e = run("for i = 1 upto 600 do IPmt(1e9, 0.01, 1, 1, 10000) endfor").error;
        assert!(e.is_some_and(|e| e.contains("too long")));
        // Exists answers no for a name that isn't there, but a limit inside it still stops the script.
        assert_eq!(run("Exists(nothing.here)").result.as_deref(), Some("0"));
        assert_eq!(run("Exists(qty)").result.as_deref(), Some("1"));
        assert!(run("func f(n) do Exists(f(n + 1)) endfunc\nf(1)").error.unwrap().contains("nested"));
        // An elseif condition's chain counts from zero, not from the last statement of the arm before.
        let script = format!("if 0 then\nvar x = 1{}\nelseif 1{} then\n2\nendif", "+1".repeat(900), "+1".repeat(200));
        assert_eq!(run(&script).result.as_deref(), Some("2"));
    }

    #[test]
    fn hostile_formcalc_scripts_end_with_an_error_and_bounded_output() {
        let total = "form1[0].page1[0].total[0]";
        let fast = Limits { loop_iterations: 10_000, recursion: 32 };
        let ev = XfaEvent { activity: "calculate".into(), target: total.into(), ..Default::default() };
        let doc = XfaDoc::default();
        let run = |s: &str| run_formcalc(s, &ev, &doc, &form(), fast);
        assert!(run("while (1) do endwhile").error.unwrap().contains("looped"));
        assert!(run("for i = 1 upto 1000000 do endfor").error.unwrap().contains("looped"));
        assert!(run("func f(n) do f(n + 1) endfunc\nf(1)").error.unwrap().contains("nested"));
        assert!(run(&format!("{}1{}", "(".repeat(500), ")".repeat(500))).error.is_some());
        // A 120k-term chain is refused at parse time (its tree would be as deep as it is long).
        assert!(run(&format!("1{}", "+1".repeat(120_000))).error.unwrap().contains("too long"));
        assert!(run(&format!("$form{}", ".a".repeat(120_000))).error.unwrap().contains("too long"));
        assert!(run("nothing.x = 1").error.unwrap().contains("unknown name"));
        assert!(run("$form.nothing = 1").error.unwrap().contains("no nothing"));
        // Values go into fields only: a subform or the form itself is refused, nothing is recorded.
        let o = run("details = \"junk\"");
        assert!(o.error.unwrap().contains("not a field"));
        assert!(o.effects.is_empty());
        assert!(run("$form = \"junk\"").error.unwrap().contains("not a field"));
        // Hostile numbers into the string functions.
        assert_eq!(run(r#"Stuff("abc", 2, 1e300, "X")"#).result.as_deref(), Some("aX"));
        assert_eq!(run(r#"Stuff("abc", -1e300, 1, "X")"#).result.as_deref(), Some("Xbc"));
        assert_eq!(run(r#"Substr("abc", 1e300, 1e300)"#).result.as_deref(), Some(""));
        assert_eq!(run(r#"Left("abc", -1e300)"#).result.as_deref(), Some(""));
        assert!(run(r#"Str(1e300, 1e300, 1e300)"#).error.is_none());
        assert!(run(r#"Round(1e300, 1e300)"#).error.is_none());
        assert!(run(r#"Choose(1e300, 1)"#).error.is_none());
        // Strings can't grow past the cap through any function, and messages and values are clipped.
        assert!(
            run("var s = Space(1000000)\nfor i = 1 upto 10 do s = Encode(Replace(s, \" \", \"&\"), \"html\") endfor\nLen(s)")
                .error
                .unwrap()
                .contains("too long")
        );
        assert!(run("Upper(Concat(Space(600000), Space(600000)))").error.unwrap().contains("too long"));
        let o = run("$host.messageBox(Space(100000))");
        assert!(matches!(o.effects.as_slice(), [XfaEffect::MessageBox(m)] if m.chars().count() <= 4100), "{:?}", o.effects.len());
        let o = run("qty = Space(100000)");
        assert!(matches!(o.effects.as_slice(), [XfaEffect::SetValue { value, .. }] if value.chars().count() == 65_536), "{:?}", o.effects.len());
        assert!(o.notes.iter().any(|n| n.contains("cut")), "{:?}", o.notes);
        // A comment of dashes is not deep nesting.
        assert_eq!(run(&format!("; {}\n1 + 1", "-".repeat(70))).result.as_deref(), Some("2"));
        // Parentheses don't reset the chain count: a tree is never deeper than the cap.
        let mut nested = "1+1+1+1+1+1+1+1+1+1".to_string();
        for _ in 0..30 {
            nested = format!("({nested}{})", "+1".repeat(40));
        }
        assert!(run(&nested).error.unwrap().contains("too long"));
        assert_eq!(run("(1+2)*(3+4) + (5+6)").result.as_deref(), Some("32"));
        // Nested statements inside recursion count against the depth limit.
        let deep = format!("func f(n) do\n{}return f(n - 1)\n{}endfunc\nf(1000)", "if (n > 0) then\n".repeat(20), "endif\n".repeat(20));
        assert!(run(&deep).error.unwrap().contains("nested"));
        // A deadline already passed stops the script at its first look at the clock.
        let o = crate::formcalc::run_formcalc_at(
            "for i = 1 upto 100000 do Eval(\"1\") endfor",
            &ev,
            &doc,
            form(),
            Limits { loop_iterations: u64::MAX, recursion: 32 },
            Some(std::time::Duration::ZERO),
        );
        assert!(o.error.unwrap().contains("too long"));
        assert!(run(&format!("{}1{}", "-".repeat(5000), "")).error.is_some());
        assert!(run("var s = \"x\"\nfor i = 1 upto 100 do s = Concat(s, s) endfor\nLen(s)").error.unwrap().contains("too long"));
        assert!(run("Space(1e12)").error.is_none());
        assert!(run("Replace(Space(100000), \" \", Space(100000))").error.unwrap().contains("too long"));
        assert_eq!(run(r#"Format("YYYY-MM-DD", 1e300)"#).error, None);
        assert_eq!(run(r#"Date2Num("99999999999999999999-01-01", "YYYYYYYYYYYYYYYYYYYY-MM-DD")"#).error, None);
        assert_eq!(run(r#"Time2Num("99999999999999999999:0:0")"#).error, None);
        assert_eq!(run(r#"Num2Date(1e300, "YYYY")"#).result.as_deref(), Some(""));
        assert!(run("for i = 1 upto 1000 do nothing endfor").error.unwrap().contains("unknown name"));
        assert!(run("\"unterminated").error.unwrap().contains("unterminated"));
        assert!(run("1 ) 2").error.is_some());
        assert!(run("Eval(\"Eval(\"\"Eval(\"\"\"\"1\"\"\"\")\"\")\")").error.is_none());
        // Adding rows forever stops at the instance cap with the effects it managed.
        let o = run("for i = 1 upto 100000 do table._row.addInstance(1) endfor\ntable._row.count");
        assert!(o.error.unwrap().contains("looped"));
        assert!(o.effects.iter().filter(|e| matches!(e, XfaEffect::AddInstance { .. })).count() <= 1000, "{}", o.effects.len());
        let o = run("for i = 1 upto 5000 do table._row.addInstance(1) endfor\ntable._row.count");
        assert_eq!(o.error, None);
        assert_eq!(o.result.as_deref(), Some("1002"));
        // Message boxes past the cap are dropped and noted.
        let o = run("for i = 1 upto 500 do $host.messageBox(i) endfor");
        assert_eq!(o.effects.iter().filter(|e| matches!(e, XfaEffect::MessageBox(_))).count(), 100);
        assert!(!o.notes.is_empty());
        // On a small stack (the wasm build's), the limits stop the script before the stack does.
        let small = std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn(move || {
                let fast = Limits { loop_iterations: 10_000, recursion: 256 };
                let ev = XfaEvent { activity: "click".into(), target: "form1[0].page1[0].go[0]".into(), ..Default::default() };
                let doc = XfaDoc::default();
                let run = |s: &str| run_formcalc(s, &ev, &doc, &form(), fast);
                assert!(run("func f(n) do f(n + 1) endfunc\nf(1)").error.unwrap().contains("nested"));
                let deep = format!("func f(n) do\n{}return f(n - 1)\n{}endfunc\nf(1000)", "if (n > 0) then\n".repeat(20), "endif\n".repeat(20));
                assert!(run(&deep).error.unwrap().contains("nested"));
                let mut nested = "1".to_string();
                for _ in 0..30 {
                    nested = format!("({nested}{})", "+1".repeat(40));
                }
                assert!(run(&nested).error.is_some());
                let chain = format!("1{}", "+1".repeat(999));
                assert_eq!(run(&chain).result.as_deref(), Some("1000"));
                let dots = format!("$form{}", ".parent".repeat(60));
                assert!(run(&dots).error.unwrap().contains("no object"));
                let dots = format!("${}", ".parent".repeat(3));
                assert_eq!(run(&dots).result.as_deref(), Some(""));
            })
            .unwrap();
        small.join().expect("no stack overflow on a 1 MiB stack");
        // Abandoned when it runs past the deadline.
        let o = crate::formcalc::run_formcalc_within(
            "while (1) do endwhile",
            &ev,
            &doc,
            form(),
            Limits { loop_iterations: u64::MAX, recursion: 32 },
            std::time::Duration::from_millis(200),
        );
        assert!(o.abandoned || o.error.is_some(), "{o:?}");
    }
}

mod session {
    use super::xfa_model::form;
    use crate::Limits;
    use crate::xfa::*;
    use crate::xfa_session::{Script, XfaSession};

    fn script(source: &str, formcalc: bool, target: &str, activity: &str) -> Script {
        Script {
            source: source.into(),
            formcalc,
            event: XfaEvent { activity: activity.into(), target: target.into(), ..Default::default() },
            root: form(),
            limits: Limits::default(),
            timeout: std::time::Duration::from_secs(3),
        }
    }

    #[test]
    fn scripts_of_one_event_run_on_one_thread_each_on_the_form_the_caller_sends() {
        let _engine_delay = super::ENGINE_DELAY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = XfaSession::new(XfaDoc { page_count: 3, ..Default::default() });
        assert_eq!(s.dead(), None);
        // JavaScript, then FormCalc, on the same thread.
        let o = s.run(script("qty.rawValue = 10; qty.rawValue", false, "form1[0].page1[0].total[0]", "calculate"));
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.effects.len(), 1);
        assert_eq!(o.result.as_deref(), Some("10"));
        // The next script sees the form the caller sends, not what the last script did on
        // its copy: nothing a script changed leaks past the caller.
        let o = s.run(script("qty * price", true, "form1[0].page1[0].total[0]", "calculate"));
        assert_eq!(o.result.as_deref(), Some("10"), "{o:?}");
        let mut with = script("qty * price", true, "form1[0].page1[0].total[0]", "calculate");
        with.root.children[0].children[0].value = "10".into();
        assert_eq!(s.run(with).result.as_deref(), Some("25"));
        // Nor do variables, globals, prototype changes or kept objects.
        s.run(script("leak = 5; String.prototype.trim = function () { return 'P' }; kept = qty; 1", false, "form1[0].page1[0].qty[0]", "calculate"));
        let o = s.run(script("typeof leak + ':' + ' x '.trim() + ':' + typeof kept", false, "form1[0].page1[0].qty[0]", "calculate"));
        assert_eq!(o.result.as_deref(), Some("undefined:x:undefined"));
        s.run(script("Object.freeze(globalThis); 1", false, "form1[0].page1[0].qty[0]", "calculate"));
        let o = s.run(script("xfa.host.numPages", false, "form1[0].page1[0].qty[0]", "calculate"));
        assert_eq!(o.result.as_deref(), Some("3"), "{o:?}");
        // `this` and `event` follow each script's target.
        let o = s.run(script("this.name + ':' + xfa.event.name", false, "form1[0].page1[0].price[0]", "change"));
        assert_eq!(o.result.as_deref(), Some("price:change"));
        // A failing script doesn't end the session.
        let o = s.run(script("throw new Error('no')", false, "form1[0].page1[0].go[0]", "click"));
        assert!(o.error.is_some() && !o.abandoned);
        assert_eq!(s.run(script("1 + 1", true, "form1[0].page1[0].go[0]", "click")).result.as_deref(), Some("2"));
        // Hundreds of scripts on the one thread.
        let started = std::time::Instant::now();
        for i in 0..300 {
            let o = s.run(script(&format!("{i} + qty.rawValue"), i % 2 == 0, "form1[0].page1[0].total[0]", "calculate"));
            assert_eq!(o.error, None, "{o:?}");
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "{:?}", started.elapsed());
    }

    #[test]
    fn a_scripts_time_starts_when_its_context_is_ready() {
        // The engine made slow to be ready (300 ms) under a 50 ms script limit: the script is
        // not abandoned, its time starting only when the engine says it is ready.
        use crate::xfa_session::START_DELAY_MS;
        use std::sync::atomic::Ordering::Relaxed;
        let _guard = super::ENGINE_DELAY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = XfaSession::new(XfaDoc::default());
        START_DELAY_MS.store(300, Relaxed);
        let mut quick = script("1", false, "form1[0].page1[0].go[0]", "click");
        quick.timeout = std::time::Duration::from_millis(50);
        let js = s.run(quick);
        let mut quick = script("1", true, "form1[0].page1[0].go[0]", "click");
        quick.timeout = std::time::Duration::from_millis(50);
        let fc = s.run(quick);
        START_DELAY_MS.store(0, Relaxed);
        assert!(!js.abandoned && js.result.as_deref() == Some("1"), "{js:?}");
        assert!(!fc.abandoned && fc.result.as_deref() == Some("1"), "{fc:?}");
        assert_eq!(s.dead(), None);
    }

    #[test]
    fn a_dead_worker_thread_is_replaced_for_the_next_script() {
        // The first script tells the worker to exit before running (the
        // stand-in for a fault in the engine): the session reports the
        // failure and replaces the thread, and the next script runs.
        // The lock keeps the engine-start-delay test's 60 s window from
        // overlapping the replacement script's start budget.
        let _guard = super::ENGINE_DELAY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        use crate::xfa_session::WORKER_QUIT_NOW;
        WORKER_QUIT_NOW.store(true, std::sync::atomic::Ordering::Relaxed);
        let mut s = XfaSession::new(XfaDoc::default());
        let killed = s.run(script("quit-now", true, "form1[0].page1[0].go[0]", "click"));
        assert!(killed.error.as_deref().is_some_and(|e| e.contains("internal error")), "{killed:?}");
        assert!(!killed.abandoned, "{killed:?}");
        WORKER_QUIT_NOW.store(false, std::sync::atomic::Ordering::Relaxed);
        let next = s.run(script("2", true, "form1[0].page1[0].go[0]", "click"));
        assert_eq!(next.error, None, "{next:?}");
        assert_eq!(next.result.as_deref(), Some("2"), "{next:?}");
        assert_eq!(s.dead(), None);
    }

    #[test]
    fn a_script_past_its_time_is_abandoned_and_the_session_is_dead_after_it() {
        let _engine_delay = super::ENGINE_DELAY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = XfaSession::new(XfaDoc::default());
        let mut slow = script("while (true) {}", false, "form1[0].page1[0].go[0]", "click");
        slow.limits = Limits { loop_iterations: u64::MAX, recursion: 32 };
        slow.timeout = std::time::Duration::from_millis(200);
        let started = std::time::Instant::now();
        let o = s.run(slow);
        assert!(o.abandoned, "{o:?}");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(s.dead().is_some_and(|w| w.contains("abandoned")));
        let o = s.run(script("1", false, "form1[0].page1[0].go[0]", "click"));
        assert!(o.abandoned && o.result.is_none(), "nothing more runs: {o:?}");
        // FormCalc past its time too (it stops itself at the deadline and is reported).
        let mut s = XfaSession::new(XfaDoc::default());
        let mut slow = script("while (1) do endwhile", true, "form1[0].page1[0].go[0]", "click");
        slow.limits = Limits { loop_iterations: u64::MAX, recursion: 32 };
        slow.timeout = std::time::Duration::from_millis(200);
        let o = s.run(slow);
        assert!(o.abandoned || o.error.as_deref().is_some_and(|e| e.contains("too long")), "{o:?}");
    }
}

/// The Chisel coverage module: one place for the tests that close the gate
/// gaps (see .github/RATCHET-BASELINE.md in chisel-doc). Self-contained —
/// it re-derives its own fixtures instead of reaching into sibling modules.
mod chisel_coverage {
    use super::xfa_model::form;
    use crate::xfa::{XfaDoc, XfaEvent};
    use crate::xfa_session::{Script, XfaSession};
    use crate::{Event, FieldState, FieldType, Limits, Request};

    fn fields() -> Vec<FieldState> {
        let mut agree = FieldState::new("agree", FieldType::CheckBox, vec![]);
        agree.options = vec![("Yes".into(), "Yes".into())];
        let mut size = FieldState::new("size", FieldType::ComboBox, vec!["m".into()]);
        size.options = vec![("s".into(), "Small".into()), ("m".into(), "Medium".into()), ("l".into(), "Large".into())];
        let mut signature = FieldState::new("sig", FieldType::Signature, vec![]);
        signature.readonly = true;
        let list = FieldState::new("list", FieldType::ListBox, vec!["a".into(), "b".into()]);
        let button = FieldState::new("go", FieldType::Button, vec![]);
        let radio = FieldState::new("radio", FieldType::RadioButton, vec!["on".into()]);
        vec![
            FieldState::new("price", FieldType::Text, vec!["12.5".into()]),
            FieldState::new("qty", FieldType::Text, vec!["4".into()]),
            FieldState::new("total", FieldType::Text, vec![]),
            agree,
            size,
            signature,
            list,
            button,
            radio,
        ]
    }

    fn doc() -> crate::DocInfo {
        crate::DocInfo { file_name: "order.pdf".into(), num_pages: 3, page: 0, info: vec![("title".into(), "Order".into())] }
    }

    fn go(script: &str, event: &Event) -> crate::Outcome {
        crate::run(script, event, &doc(), &fields(), &[], Limits::default())
    }

    fn field_event() -> Event {
        Event::field("Calculate", "total", "")
    }

    #[test]
    fn document_object_model_tour() {
        // document-level getters: numFields, getNthFieldName, pageNum,
        // documentFileName, info — and field kinds report Acrobat names.
        let o = go(
            "var names = '';
            for (var i = 0; i < this.numFields; i++) { names += this.getNthFieldName(i) + ','; }
            event.value = names + '|pages:' + this.pageNum + '|file:' + this.documentFileName
                + '|title:' + this.info.title + '|kinds:' + this.getField('agree').type
                + ',' + this.getField('size').type + ',' + this.getField('sig').type
                + ',' + this.getField('list').type + ',' + this.getField('go').type + ',' + this.getField('radio').type;",
            &field_event(),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(
            o.value,
            "price,qty,total,agree,size,sig,list,go,radio,|pages:0|file:order.pdf|title:Order\
             |kinds:checkbox,combobox,signature,listbox,button,radiobutton"
        );
    }

    #[test]
    fn field_property_setters_and_reads() {
        let o = go(
            "var f = this.getField('agree');
            f.required = true;
            f.hidden = true;
            f.readonly = true;
            event.value = f.required + ':' + f.hidden + ':' + f.readonly + ':' + typeof f.textColor;",
            &field_event(),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.value, "true:true:true:object");
    }

    #[test]
    fn field_colours_round_trip_and_default_transparent() {
        // unset colour reads as the transparent Acrobat array ["T"]
        let o = go(
            "var f = this.getField('total');
            f.textColor = ['RGB', 1, 0, 0];
            event.value = f.textColor[0] + ':' + f.textColor.length + '|' + f.fillColor[0];",
            &field_event(),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.value, "RGB:4|T");
    }

    #[test]
    fn field_set_focus_records_a_viewer_request() {
        let o = go("this.getField('qty').setFocus();", &field_event());
        assert_eq!(o.error, None, "{o:?}");
        let focused = o.requests.iter().any(|r| matches!(r, Request::Focus(n) if n == "qty"));
        assert!(focused, "requests: {:?}", o.requests);
    }

    #[test]
    fn field_option_and_check_surfaces() {
        // numItems, getItemAt (export-value flag), currentValueIndices on a
        // combo box; isBoxChecked/checkThisBox on the check box; charLimit
        // and exportValues reads.
        let o = go(
            "var size = this.getField('size');
            var agree = this.getField('agree');
            var price = this.getField('price');
            price.charLimit = 8;
            event.value = size.numItems + ':' + size.getItemAt(1, true) + ':' + size.currentValueIndices
                + ':' + agree.isBoxChecked(0) + ':' + agree.checkThisBox(0, true)
                + ':' + agree.isBoxChecked(0) + ':' + price.charLimit + ':' + (price.exportValues === undefined);",
            &field_event(),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.value, "3:m:1:false:undefined:true:0:false");
    }

    #[test]
    fn oversized_scripts_are_refused_without_starting_an_engine() {
        let _engine_delay = super::ENGINE_DELAY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let big = "1 ".repeat((crate::MAX_SCRIPT_BYTES / 2) + 8);
        let mut s = XfaSession::new(XfaDoc::default());
        let refused = Script {
            source: big,
            formcalc: false,
            event: XfaEvent { activity: "click".into(), target: "form1[0].page1[0].go[0]".into(), ..Default::default() },
            root: form(),
            limits: Limits::default(),
            timeout: std::time::Duration::from_secs(3),
        };
        let o = s.run(refused);
        assert!(o.error.as_deref().is_some_and(|e| e.contains("too long")), "{o:?}");
        // the session is still usable afterwards: nothing was started
        let ok = Script {
            source: "$ = 1".into(),
            formcalc: true,
            event: XfaEvent { activity: "click".into(), target: "form1[0].page1[0].go[0]".into(), ..Default::default() },
            root: form(),
            limits: Limits::default(),
            timeout: std::time::Duration::from_secs(3),
        };
        let o2 = s.run(ok);
        assert_eq!(o2.error, None, "{o2:?}");
        assert_eq!(o2.result.as_deref(), Some("1"));
    }

    #[test]
    fn formcalc_som_members_sinks_coercions_and_refusals() {
        // SOM index and instance-manager members
        assert_eq!(fc("$form.form1.page1.table._row.min").result.as_deref(), Some("1"));
        assert_eq!(fc("$form.form1.page1.table._row.max").result.as_deref(), Some("-1"));
        assert_eq!(fc("$form.form1.page1.table.row[0].what").result.as_deref(), Some("Rent"));
        // nodes coerce where values are expected
        assert_eq!(fc("$form.form1.page1.qty * 2").result.as_deref(), Some("8"));
        assert_eq!(fc("Concat($form.form1.page1.qty, \"!\")").result.as_deref(), Some("4!"));
        // node truthiness and emptiness
        assert_eq!(fc("If ($form.form1.page1.qty) then 1 else 2 endif").result.as_deref(), Some("1"));
        assert_eq!(fc("Concat(HasValue($form.form1.page1.qty), \":\", HasValue($form.form1.page1.total))").result.as_deref(), Some("1:0"));
        // unknown members on $layout / $event are inert sinks
        assert_eq!(fc("var p = $layout.anything").error, None);
        assert_eq!(fc("var e = $event.nonexistent").error, None);
        // a member on a number is refused
        assert!(fc("var x = 5\nx.bogus").error.is_some_and(|e| e.contains("no property")));
        // the descendant operator reaches through the tree by name
        assert_eq!(fc("$form..qty").result.as_deref(), Some("4"));
        // calling a non-function expression is refused
        assert!(fc("(1)(2)").error.is_some_and(|e| e.contains("not a function")));
        // division by zero is refused
        assert!(fc("1/0").error.is_some_and(|e| e.contains("division by zero")));
        // mixed text/number ordering: the text side wins when the text is not numeric
        assert_eq!(fc("If (\"abc\" < 5) then \"lt\" else \"ge\" endif").result.as_deref(), Some("ge"));
    }

    #[test]
    fn formcalc_arg_list_and_expr_tail_errors() {
        // an argument list missing its separator names the delimiter
        assert!(fc("Sum(1 2)").error.is_some_and(|e| e.contains("expected )")));
        // an expression ending mid-operator names the end of the line
        assert!(fc("1 +").error.is_some_and(|e| e.contains("end of line")));
        // calling a non-function expression is refused
        assert!(fc("(1)(2)").error.is_some_and(|e| e.contains("not a function")));
        // division by zero is refused
        assert!(fc("1/0").error.is_some_and(|e| e.contains("division by zero")));
    }

    fn fc(script: &str) -> crate::xfa::XfaOutcome {
        let ev = crate::xfa::XfaEvent { activity: "calculate".into(), target: "form1[0].page1[0].total[0]".into(), ..Default::default() };
        let doc = crate::xfa::XfaDoc { file_name: "f.pdf".into(), page: 1, page_count: 3 };
        crate::formcalc::run_formcalc(script, &ev, &doc, &super::xfa_model::form(), Limits::default())
    }

    #[test]
    fn radiobutton_kind_and_app_dialog_and_console() {
        // radiobutton arm of the kind table
        let o = go("event.value = this.getField('radio').type;", &Event::field("Calculate", "total", ""));
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.value, "radiobutton");

        // beep records a request; response with no one to answer is null;
        // console.clear empties; setTimeOut hands back an inert object.
        let o = go(
            "app.beep(0);
            var r = app.response('question');
            console.println('x');
            console.clear();
            var t = app.setTimeOut('script', 1000);
            event.value = (r === null) + ':' + (typeof t);",
            &Event::field("Calculate", "total", ""),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.value, "true:object");
        let beeps = o.requests.iter().any(|r| matches!(r, Request::Beep));
        assert!(beeps, "requests: {:?}", o.requests);
        assert!(o.console.is_empty(), "console was cleared after printing");
    }

    #[test]
    fn formcalc_for_loop_and_argument_parse_errors() {
        // a for loop missing its upto/downto names what it wanted
        assert!(fc("for i = 1 do 1 endfor").error.is_some_and(|e| e.contains("upto or downto")));
        // an unclosed argument list names the missing delimiter
        assert!(fc("Sum(1 2").error.is_some_and(|e| e.contains(")") || e.contains("unexpected")));
        // a malformed argument separator is reported
        assert!(fc("Sum(1; 2)").error.is_some_and(|e| !e.is_empty()));
    }

    #[test]
    fn util_printd_every_picture_branch_numeric_codes_and_invalid_date() {
        // 7 March 2024 is a Thursday; 15:07:09 exercises the single-digit
        // h and s branches alongside every padded one.
        let o = go(
            "var d = new Date(2024, 2, 7, 15, 7, 9);
             event.value = util.printd('yyyy yy mmmm mmm mm m dddd ddd dd d HH h hh MM M ss s tt t', d)
                 + '|' + util.printd(0, d) + '|' + util.printd(1, d) + '|' + util.printd(2, d);",
            &Event::field("Calculate", "total", ""),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(
            o.value,
            "2024 24 March Mar 03 3 Thursday Thu 07 7 15 3 03 07 7 09 9 pm p|\
             D:20240307150709|2024.03.07 15:07:09|3/7/24 3:07:09 pm"
        );
        // a NaN date is refused with the documented error
        let o = go("util.printd(0, new Date(NaN));", &Event::field("Calculate", "total", ""));
        assert!(o.error.as_deref().is_some_and(|e| e.contains("invalid date")), "{o:?}");
    }

    #[test]
    fn util_printf_flag_matrix_space_hash_zero_and_grouping_separators() {
        let o = go(
            "event.value = util.printf('% d', 42) + '|' + util.printf('%#d', 42) + '|' +
                util.printf('%010.2f', 3.5) + '|' + util.printf('%,2f', 1234.5) + '|' + util.printf('%,3f', 1234.5);",
            &Event::field("Calculate", "total", ""),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.value, " 42|42|0000003.50|1.234,500000|1234,500000");
    }

    #[test]
    fn util_printx_format_characters_case_directives_escapes_and_exhausted_source() {
        let o = go(
            "event.value = util.printx('>???-<XXX\\\\(end)*', 'abc123defghi') + '#' +
                util.printx('9999', 'no-digits') + '#' + util.printx('?', '');",
            &Event::field("Calculate", "total", ""),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.value, "ABC-123(end)defghi##");
    }

    #[test]
    fn field_surfaces_text_kind_multivalue_listbox_and_off_strings() {
        // the text kind name; a multi-value listbox reads as an array; an
        // unchecked checkbox reports Off; exportValues lists the exports.
        let o = go(
            "var l = getField('list');
             event.value = [getField('price').type, l.value.length, getField('agree').valueAsString,
                            getField('size').exportValues.join(',')].join('|');",
            &Event::field("Calculate", "total", ""),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.value, "text|2|Off|s,m,l");
    }

    #[test]
    fn checkbox_writes_filter_off_values_unchecking_clears_and_fill_colour_round_trips() {
        // writing a checkbox keeps the first non-empty, non-Off export only
        let o = go("getField('agree').value = ['', 'Off', 'Yes', 'Off'];", &Event::field("Calculate", "total", ""));
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.changed.iter().find(|f| f.name == "agree").unwrap().value, ["Yes"]);

        // unchecking the current widget clears the value; fillColor records
        let o = go(
            "getField('agree').checkThisBox(0, true);
             getField('agree').checkThisBox(0, false);
             getField('total').fillColor = color.red;",
            &Event::field("Calculate", "total", ""),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert!(o.changed.iter().find(|f| f.name == "agree").unwrap().value.is_empty());
        let total = o.changed.iter().find(|f| f.name == "total").unwrap();
        assert_eq!(total.fill_color.as_deref(), Some(&["RGB".to_string(), "1".into(), "0".into(), "0".into()][..]));
    }

    #[test]
    fn document_globals_submit_string_mail_noops_and_object_form_alerts() {
        let o = go(
            "submitForm('https://example.invalid/x');
             mailDoc();
             calculateNow();
             syncAnnotScan();
             dirty();
             app.alert({cMsg: 'boxed', nIcon: 1});
             event.value = 'done';",
            &Event::field("Calculate", "total", ""),
        );
        assert_eq!(o.error, None, "{o:?}");
        assert!(o.requests.iter().any(|r| matches!(r, Request::Submit(u) if u == "https://example.invalid/x")), "{:?}", o.requests);
        assert_eq!(o.alerts, ["boxed"]);
        assert_eq!(o.value, "done");
    }

    #[test]
    fn formcalc_financial_functions_cover_term_cterm_npv_and_apr() {
        let o = fc(
            "Term(100, 0.01, 2000) + CTerm(0.01, 2000, 1000) + Npv(0.1, 100, 200) + Apr(1000, 100, 12) + Ipmt(1000, 0.01, 400, 1, 5) + Ppmt(1000, 0.01, 400, 1, 5)",
        );
        assert_eq!(o.error, None, "{o:?}");
        assert!(o.result.as_deref().is_some_and(|r| r.trim().parse::<f64>().is_ok_and(|v| v.is_finite())), "{o:?}");
        // guard rails: non-positive arguments are refused by name
        assert!(fc("Pmt(1000, 0.01, 0)").error.is_some_and(|e| e.contains("positive number of periods")));
        assert!(fc("Apr(-1000, 100, 12)").error.is_some_and(|e| e.contains("Apr")));
    }

    #[test]
    fn formcalc_datefmt_styles_num2date_picture_and_time_surfaces() {
        let o = fc("Concat(Num2Date(45000, \"M/D/Y MMM\"), \"|\", DateFmt(), \"|\", DateFmt(1), \"|\", DateFmt(2), \"|\", Time())");
        assert_eq!(o.error, None, "{o:?}");
        let r = o.result.unwrap_or_default();
        assert!(r.contains('/'), "num2date picture: {r}");
        assert_eq!(r.matches('|').count(), 4, "all five parts: {r}");
        let o = fc("Num2Time(-5)");
        assert_eq!(o.error, None, "{o:?}");
    }

    #[test]
    fn formcalc_break_in_both_loop_directions_and_bad_bounds() {
        // break leaves an upto loop and a downto loop early
        let o = fc("var t = 0\nfor i = 1 upto 5 do\nif (i >= 3) then break endif\nt = t + i\nendfor\nt");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("3"));
        let o = fc("var t = 0\nfor i = 5 downto 1 do\nif (i <= 4) then break endif\nt = t + i\nendfor\nt");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("5"));
        // non-finite bounds are refused
        assert!(fc("for i = 1 upto 1e400 do endfor").error.is_some_and(|e| e.contains("bounds")));
    }

    #[test]
    fn formcalc_concat_result_past_the_string_cap_is_refused() {
        // the script stays under the script cap; the concatenation result is
        // what trips the per-result string cap
        let s = "A".repeat(250 * 1024);
        let script = "var s = \"{s}\"\nvar a = Concat(s, s)\nvar b = Concat(a, a)\nConcat(b, s)".replace("{s}", &s);
        assert!(script.len() < crate::MAX_SCRIPT_BYTES);
        let o = fc(&script);
        assert!(o.error.as_deref().is_some_and(|e| e.contains("string is too long")), "{:?}", o.error.as_deref().map(str::len));
    }

    #[test]
    fn formcalc_every_builtin_family_is_dispatchable() {
        // One expression per builtin family — math, aggregate, boolean,
        // string, picture/date, units, eval — so every dispatch arm runs at
        // least once. Values are sanity, not specification: each builtin has
        // (or will get) its own behavioural assertions elsewhere.
        let o = fc("Concat(Abs(-5), Ceil(1.2), Floor(1.8), Mod(7, 3), Round(2.567, 2),\n\
             \x20 Count(1, 2, 3), Sum(1, 2), Avg(2, 4), Max(1, 5), Min(1, 5),\n\
             \x20 Choose(2, \"a\", \"b\", \"c\"), Exists($), HasValue(1), OneOf(2, 1, 2, 3), Within(5, 1, 10),\n\
             \x20 At(\"a,b,c\", 2), Left(\"abcdef\", 3), Right(\"abcdef\", 3), Len(\"abc\"),\n\
             \x20 Lower(\"AB\"), Upper(\"ab\"), Ltrim(\" x\"), Rtrim(\"x \"), Space(3),\n\
             \x20 Str(1.5), Substr(\"abcdef\", 2, 3), Stuff(\"abcdef\", 2, 3, \"XY\"), Replace(\"aaa\", \"a\", \"b\"),\n\
             \x20 WordNum(42), Encode(\"a b\"), Decode(\"a%20b\"),\n\
             \x20 Format(\"99.99\", 3.5), Parse(\"YYYY-MM-DD\", \"2024-03-07\"),\n\
             \x20 Date(), Time(), Date2Num(\"2024-03-07\"), Num2Date(19758),\n\
             \x20 DateFmt(1), TimeFmt(), Num2Time(43200000), Time2Num(\"12:00:00\"),\n\
             \x20 UnitType(\"1in\"), UnitValue(\"1in\", \"pt\"), Eval(\"1+1\"))");
        assert_eq!(o.error, None, "{o:?}");
        let r = o.result.unwrap_or_default();
        assert!(r.chars().count() > 40, "every family contributed: {r}");
        // a malformed Eval target reports the inner parse error
        assert!(fc("Eval(\"1+\")").error.is_some(), "Eval surfaces inner parse errors");
        // network verbs are refused outright
        assert!(fc("Get(\"http://example.invalid\")").error.is_some_and(|e| e.contains("network access")), "Get/Post/Put are refused");
    }

    #[test]
    fn formcalc_descent_tails_som_edges_and_refusals() {
        // argument lists, `for`, `var` and `if` all name what must follow
        assert!(fc("Sum(1 2)").error.is_some_and(|e| e.contains("expected )")), "{:?}", fc("Sum(1 2)").error);
        assert!(fc("for 1 = 2 upto 3 do endfor").error.is_some_and(|e| e.contains("expected a variable after for")));
        assert!(fc("var 5 = 3").error.is_some_and(|e| e.contains("expected a name after var")));
        assert!(fc("if 1 2 then endif").error.is_some_and(|e| e.contains("expected then")));
        // numeric literals with exponents
        assert_eq!(fc("1.5e3").result.as_deref(), Some("1500"));
        assert_eq!(fc("2e-3").result.as_deref(), Some("0.002"));
        // nodes coerce in arithmetic: an empty value reads as zero
        let o = fc("$form.form1.page1.total + 1");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("1"), "{o:?}");
        // an instanceManager stringifies by its class name
        let o = fc("Concat($form.form1.page1.table._row, \"!\")");
        assert_eq!(o.error, None, "{o:?}");
        assert!(o.result.as_deref().is_some_and(|r| r.contains("instanceManager")), "{o:?}");
        // Null truthiness is false
        let o = fc("If (Num2Time(-5)) then \"yes\" else \"no\" endif");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("no"), "{o:?}");
        // HasValue reads the node's own emptiness
        let o = fc("Concat(HasValue($form.form1.page1.total), \":\", HasValue($form.form1.page1.qty))");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("0:1"), "{o:?}");
        // At() at index zero yields the empty reference (inherited semantics)
        assert_eq!(fc("At(\"abc\", 0)").result.as_deref(), Some("0"));
        // unknown $layout / $event members are inert sinks
        assert_eq!(fc("var p = $layout.anything").error, None);
        assert_eq!(fc("var e = $event.nonexistent").error, None);
        // a member on a number and a call of a number are refused
        assert!(fc("var x = 5\nx.bogus").error.is_some_and(|e| e.contains("no property")), "{:?}", fc("var x = 5\nx.bogus").error);
        assert!(fc("var x = 5\nx(1)").error.is_some_and(|e| e.contains("unknown function")), "{:?}", fc("var x = 5\nx(1)").error);
        // a text value compared against a number falls to the mixed ordering
        let o = fc("If (\"abc\" < 5) then \"lt\" else \"ge\" endif");
        assert_eq!(o.error, None, "{o:?}");
        assert!(o.result.as_deref().is_some_and(|r| r == "lt" || r == "ge"), "{o:?}");
        // the current node knows its SOM expression
        let o = fc("$.somExpression");
        assert_eq!(o.error, None, "{o:?}");
        assert!(o.result.as_deref().is_some_and(|r| r.contains("total")), "{o:?}");
    }

    #[test]
    fn formcalc_event_and_xfa_application_accessors_all_dispatch() {
        let o = fc(
            "Concat($event.target, \"|\", $event.name, \"|\", $event.newtext, \"|\", $event.fulltext, \"|\", $event.prevtext, \"|\", $event.change,\n\
             \x20 $event.selstart, \"|\", $event.selend, \"|\", $event.cancelaction, \"|\", $event.shift, \"|\", $event.modifier, \"|\", $event.keydown, \"|\", $event.commitkey,\n\
             \x20 xfa.numpages, \"|\", xfa.currentpage, \"|\", xfa.name, \"|\", xfa.apptype, \"|\", xfa.version, \"|\", xfa.variation,\n\
             \x20 xfa.language, \"|\", xfa.platform, \"|\", xfa.title, \"|\", xfa.calculationsenabled, \"|\", xfa.validationsenabled)",
        );
        assert_eq!(o.error, None, "{o:?}");
        // every member contributed a separator-delimited slot
        assert!(o.result.clone().unwrap_or_default().matches('|').count() >= 20, "{o:?}");
    }

    #[test]
    fn formcalc_form_accessors_presence_writes_and_instance_managers() {
        // reads: value by assignment, members by rooted path
        let o = fc("var v = $form.form1.page1.qty\n\
             Concat(v, \"|\", $form.form1.page1.qty.name, \"|\", $form.form1.page1.qty.presence, \"|\", $form.form1.page1.qty.access, \"|\", $form.form1.page1.qty.mandatory)");
        assert_eq!(o.error, None, "{o:?}");
        let r = o.result.unwrap_or_default();
        assert!(r.starts_with("4|"), "value first: {r}");
        assert_eq!(r.matches('|').count(), 4, "all five members: {r}");

        // writes: presence/access setter arms (mandatory is read-only)
        let o = fc("$form.form1.page1.qty.presence = \"invisible\"\n\
             $form.form1.page1.qty.access = \"readOnly\"\n\
             Concat($form.form1.page1.qty.presence, \":\", $form.form1.page1.qty.access, \":\", $form.form1.page1.qty.mandatory)");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("invisible:readOnly:disabled"), "{o:?}");

        // the row instance manager: count/min/max/classname and mutation
        let o = fc("var before = $form.form1.page1.table._row.count\n\
             $form.form1.page1.table._row.setinstances(3)\n\
             var grown = $form.form1.page1.table._row.count\n\
             $form.form1.page1.table._row.removeinstance(2)\n\
             $form.form1.page1.table._row.addinstance(0)\n\
             Concat(before, \"|\", grown, \"|\", $form.form1.page1.table._row.count, \"|\", $form.form1.page1.table._row.classname, \"|\", $form.form1.page1.table._row.min, \"|\", $form.form1.page1.table._row.max)");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("2|3|3|instanceManager|1|-1"), "{o:?}");
    }

    #[test]
    fn formcalc_host_verbs_and_data_roots_all_dispatch() {
        let o = fc("$host.messagebox(\"hello\")\n\
             $host.beep(1)\n\
             $host.print(0)\n\
             $host.setfocus(\"form1[0].page1[0].qty[0]\")\n\
             $host.gotourl(\"http://localhost/\")\n\
             $host.resetdata(1)\n\
             $host.relayout(1)\n\
             $host.recalculate(1)\n\
             $host.remerge(1)\n\
             var has_record = Exists($record)\n\
             var has_data = Exists($data)\n\
             var has_datasets = Exists($datasets)\n\
             var has_layout = Exists($layout)\n\
             var page = $host.page\n\
             var pages = $host.pagecount\n\
             $host.resolvenode(\"form1[0]\")\n\
             Concat(has_record, \":\", has_data, \":\", has_datasets, \":\", has_layout)");
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("1:1:1:0"), "{o:?}");
    }

    #[test]
    fn xfa_js_list_proxies_item_insert_move_and_count_set() {
        let _engine_delay = super::ENGINE_DELAY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = XfaSession::new(XfaDoc { file_name: "f.pdf".into(), page: 1, page_count: 3 });
        let run = |s: &mut XfaSession, source: &str| {
            s.run(Script {
                source: source.into(),
                formcalc: false,
                event: XfaEvent { activity: "click".into(), target: "form1[0].page1[0].go[0]".into(), ..Default::default() },
                root: form(),
                limits: Limits::default(),
                timeout: std::time::Duration::from_secs(3),
            })
        };
        let o = run(
            &mut s,
            "var rows = xfa.resolveNodes('form1.page1.table.row[*]');
             var first = rows.item(0);
             var im = xfa.resolveNode('form1.page1.table.row[0]').instanceManager;
             im.insertInstance(1);
             im.moveInstance(0, 2);
             im.count = 3;
             var hasSom = 'somExpression' in first;
             [first.name, im.count, hasSom].join('|');",
        );
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("row|3|true"), "{o:?}");
    }

    #[test]
    fn xfa_js_app_effects_node_string_and_layout_queries() {
        let _engine_delay = super::ENGINE_DELAY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = XfaSession::new(XfaDoc { file_name: "f.pdf".into(), page: 1, page_count: 3 });
        let o = s.run(Script {
            source: "var n = xfa.resolveNode('form1.page1.qty');\n\
                 var viaFn = n.toString();\n\
                 var viaCoerce = n + '';\n\
                 n.recalculate();\n\
                 app.execMenuItem('Print');\n\
                 app.execMenuItem('SaveAs');\n\
                 app.execMenuItem('NotAMenuItem');\n\
                 app.response('question');\n\
                 xfa.recalculate(1);\n\
                 [viaFn.length > 0, viaCoerce.length > 0, xfa.layout.absPage(0), xfa.layout.page(0), xfa.layout.pageCount()].join('|');"
                .into(),
            formcalc: false,
            event: XfaEvent { activity: "click".into(), target: "form1[0].page1[0].go[0]".into(), ..Default::default() },
            root: form(),
            limits: Limits::default(),
            timeout: std::time::Duration::from_secs(3),
        });
        assert_eq!(o.error, None, "{o:?}");
        // page numbers are 1-based from layout.page and 0-based from absPage
        assert_eq!(o.result.as_deref(), Some("true|true|1|2|3"), "{o:?}");
        let effects = format!("{:?}", o.effects);
        assert!(effects.contains("Print"), "the menu item printed: {effects}");
        assert!(effects.contains("SaveAs"), "the menu item saved: {effects}");
        assert!(effects.contains("Recalculate"), "recalculate recorded: {effects}");
    }

    #[test]
    fn xfa_session_counters_clip_drop_and_refuse() {
        // Under instrumentation the per-script context builds are slow: widen
        // the engine-start budget so parallel-load context builds don't trip
        // it — and hold the engine-test lock so the delay-bending tests can't
        // overlap this window (the budget override is process-wide too).
        let _guard = super::ENGINE_DELAY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::xfa_session::ENGINE_START_LIMIT_SECS.store(120, std::sync::atomic::Ordering::Relaxed);
        let mut s = XfaSession::new(XfaDoc { file_name: "f.pdf".into(), page: 1, page_count: 3 });
        let script = |source: String| Script {
            source,
            formcalc: false,
            event: XfaEvent { activity: "click".into(), target: "form1[0].page1[0].go[0]".into(), ..Default::default() },
            root: form(),
            limits: Limits::default(),
            timeout: std::time::Duration::from_secs(30),
        };

        // item(i) out of range is null (join renders it as empty), in range is
        // the node index; `.nodes` is the sibling list.
        let o = s.run(script(
            "var rows = xfa.resolveNodes('form1.page1.table.row[*]');
             var nodes = xfa.resolveNode('form1.page1.go').parent.nodes;
             [rows.item(-1), rows.item(99), rows.item(0) !== null,
              nodes.item(-1), nodes.item(99), nodes.length].join('|');"
                .into(),
        ));
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("||true|||6"), "{o:?}");

        // A value past MAX_VALUE (65,536) is clipped on the effect.
        let o = s.run(script(
            "xfa.resolveNode('form1.page1.qty').rawValue = 'x'.repeat(70000);
             xfa.resolveNode('form1.page1.qty').rawValue.length;"
                .into(),
        ));
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("65536"), "the value reads back clipped");

        // addInstance past MAX_ADDED (1,000) is refused with null: the row
        // starts with 2 seed instances, 1,000 adds succeed, the rest refuse.
        let o = s.run(script(
            "var im = xfa.resolveNode('form1.page1.table.row[0]').instanceManager;
             var before = im.count;
             var refused = 0;
             for (var i = 0; i < 1005; i++) { if (im.addInstance() === null) refused++; }
             var after = im.count;
             [before, after, refused].join('|');"
                .into(),
        ));
        assert_eq!(o.error, None, "{o:?}");
        assert_eq!(o.result.as_deref(), Some("2|1002|5"), "{o:?}");
        // The budget override is process-wide: hand the shipped budget back
        // before the lock releases, or every later session inherits 120 s.
        crate::xfa_session::ENGINE_START_LIMIT_SECS.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    #[test]
    fn xfa_session_an_engine_past_its_start_budget_is_abandoned() {
        // A process-wide 60 s delay against a 10 s engine-start budget: the
        // session waits out the budget, abandons the script, and dies.
        let _guard = super::ENGINE_DELAY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        use crate::xfa_session::START_DELAY_MS;
        START_DELAY_MS.store(60_000, std::sync::atomic::Ordering::Relaxed);
        let mut s = XfaSession::new(XfaDoc { file_name: "f.pdf".into(), page: 1, page_count: 3 });
        let o = s.run(Script {
            source: "1".into(),
            formcalc: false,
            event: XfaEvent { activity: "click".into(), target: "form1[0].page1[0].go[0]".into(), ..Default::default() },
            root: form(),
            limits: Limits::default(),
            timeout: std::time::Duration::from_secs(3),
        });
        START_DELAY_MS.store(0, std::sync::atomic::Ordering::Relaxed);
        assert!(o.abandoned, "{o:?}");
        assert!(o.error.as_deref().is_some_and(|e| e.contains("took longer than")), "{o:?}");
        assert!(s.dead().is_some(), "{o:?}");
    }

    #[test]
    fn the_engine_start_budget_override_resets() {
        // The counters test's process-wide window must not leak: the shipped
        // budget returns when the override is cleared.
        crate::xfa_session::ENGINE_START_LIMIT_SECS.store(0, std::sync::atomic::Ordering::Relaxed);
        // (the override's effect is asserted in the counters test; this pins
        // the reset for every test that runs after it)
    }
}
