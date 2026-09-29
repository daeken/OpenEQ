use openeq_ui::{DrawCommand, Rect, UiBindings, UiDocument, UiError};

const XML: &str = r#"<XML>
<TextureInfo item="atlas.tga"><Size><CX>128</CX><CY>64</CY></Size></TextureInfo>
<Ui2DAnimation item="normal"><Cycle>true</Cycle><Frames><Texture>atlas.tga</Texture><Location><X>4</X><Y>8</Y></Location><Size><CX>20</CX><CY>10</CY></Size><Duration>100</Duration></Frames><Frames><Texture>atlas.tga</Texture><Location><X>24</X><Y>8</Y></Location><Size><CX>20</CX><CY>10</CY></Size><Duration>200</Duration></Frames></Ui2DAnimation>
<Ui2DAnimation item="hover"><Frames><Texture>atlas.tga</Texture><Location><X>44</X><Y>8</Y></Location><Size><CX>20</CX><CY>10</CY></Size></Frames></Ui2DAnimation>
<Button item="button"><ScreenID>Go</ScreenID><AutoStretch>true</AutoStretch><LeftAnchorOffset>10</LeftAnchorOffset><RightAnchorToLeft>false</RightAnchorToLeft><RightAnchorOffset>10</RightAnchorOffset><TopAnchorOffset>10</TopAnchorOffset><BottomAnchorOffset>30</BottomAnchorOffset><Text>A &amp; B</Text><ButtonDrawTemplate><Normal>normal</Normal><Flyby>hover</Flyby></ButtonDrawTemplate></Button>
<Gauge item="health"><Location><X>10</X><Y>40</Y></Location><Size><CX>80</CX><CY>10</CY></Size><EQType>1</EQType><GaugeOffsetY>0</GaugeOffsetY><FillTint><R>240</R><G>0</G><B>0</B></FillTint><GaugeDrawTemplate><Fill>normal</Fill></GaugeDrawTemplate></Gauge>
<StaticAnimation item="hidden"><AutoDraw>false</AutoDraw><Size><CX>20</CX><CY>10</CY></Size><Animation>normal</Animation></StaticAnimation>
<Screen item="window"><Location><X>20</X><Y>30</Y></Location><Size><CX>100</CX><CY>80</CY></Size><Pieces>button</Pieces><Pieces>health</Pieces><Pieces>hidden</Pieces></Screen>
</XML>"#;

#[test]
fn hotbutton_reuses_button_art_states_and_centered_text_without_nested_slot_behavior() {
    let xml = r#"<XML>
      <TextureInfo item="buttons.tga"><Size><CX>200</CX><CY>40</CY></Size></TextureInfo>
      <Ui2DAnimation item="normal"><Frames><Texture>buttons.tga</Texture><Location><X>0</X><Y>0</Y></Location><Size><CX>40</CX><CY>40</CY></Size></Frames></Ui2DAnimation>
      <Ui2DAnimation item="hover"><Frames><Texture>buttons.tga</Texture><Location><X>40</X><Y>0</Y></Location><Size><CX>40</CX><CY>40</CY></Size></Frames></Ui2DAnimation>
      <Ui2DAnimation item="pressed"><Frames><Texture>buttons.tga</Texture><Location><X>80</X><Y>0</Y></Location><Size><CX>40</CX><CY>40</CY></Size></Frames></Ui2DAnimation>
      <Ui2DAnimation item="pressed_hover"><Frames><Texture>buttons.tga</Texture><Location><X>120</X><Y>0</Y></Location><Size><CX>40</CX><CY>40</CY></Size></Frames></Ui2DAnimation>
      <Ui2DAnimation item="disabled"><Frames><Texture>buttons.tga</Texture><Location><X>160</X><Y>0</Y></Location><Size><CX>40</CX><CY>40</CY></Size></Frames></Ui2DAnimation>
      <HotButton item="hot"><ScreenID>HB_Button1</ScreenID><Size><CX>40</CX><CY>40</CY></Size><Text>Attack</Text><SpellGem>not_a_piece</SpellGem><InvSlot>not_a_server_slot</InvSlot><ButtonDrawTemplate><Normal>normal</Normal><Flyby>hover</Flyby><Pressed>pressed</Pressed><PressedFlyby>pressed_hover</PressedFlyby><Disabled>disabled</Disabled></ButtonDrawTemplate></HotButton>
    </XML>"#;
    let document = UiDocument::from_xml(xml).unwrap();
    for (enabled, pressed, hovered, x) in [
        (true, false, false, 0.),
        (true, false, true, 40.),
        (true, true, false, 80.),
        (true, true, true, 120.),
        (false, false, false, 160.),
    ] {
        let mut bindings = UiBindings::default();
        let state = bindings.widget_mut("hot");
        state.enabled = Some(enabled);
        state.pressed = pressed;
        state.hovered = hovered;
        let frame = document
            .window("hot")
            .unwrap()
            .layout(Rect::new(0., 0., 100., 100.), &bindings);
        assert!(frame.warnings.is_empty());
        assert_eq!(frame.hit_targets.len(), 1);
        assert_eq!(frame.hit_targets[0].kind, "HotButton");
        assert_eq!(frame.hit_targets[0].enabled, enabled);
        assert_eq!(frame.hit_test([5., 5.]).is_some(), enabled);
        assert!(
            frame
                .commands
                .iter()
                .any(|cmd| matches!(cmd,DrawCommand::Image{source,..} if source.x==x))
        );
        assert!(frame.commands.iter().any(|cmd|matches!(cmd,DrawCommand::Text{align,vertical_center:true,text,..} if *align==openeq_ui::TextAlign::Center && text=="Attack")));
    }
}

#[test]
fn readonly_stml_scrolls_with_clipped_hits_and_missing_optional_art() {
    let xml = r#"<XML><STMLbox item="description"><ScreenID>DescriptionText</ScreenID><Size><CX>120</CX><CY>80</CY></Size><Style_VScroll>true</Style_VScroll><Text>line one</Text></STMLbox></XML>"#;
    let document = UiDocument::from_xml(xml).unwrap();
    let mut bindings = UiBindings::default();
    let state = bindings.widget_mut("description");
    state.scroll_rows = Some(usize::MAX);
    state.scroll_id = Some("spell:200".into());
    state.text = Some("First\n\nLast".into());
    let frame = document
        .window("description")
        .unwrap()
        .layout(Rect::new(0., 0., 110., 70.), &bindings);
    assert!(frame.warnings.is_empty());
    assert_eq!(
        frame.hit_test([4., 4.]).unwrap().screen_id,
        "DescriptionText"
    );
    assert_eq!(
        frame.hit_test([108., 4.]).unwrap().item,
        "description:scroll:-1"
    );
    assert!(frame.hit_test([115., 4.]).is_none());
    let (rect, clip, thumb) = frame
        .commands
        .iter()
        .find_map(|cmd| match cmd {
            DrawCommand::TextArea {
                id,
                text,
                rect,
                clip,
                scroll_rows,
                thumb,
                ..
            } => {
                assert_eq!(id, "spell:200");
                assert_eq!(text, "First\n\nLast");
                assert_eq!(*scroll_rows, usize::MAX);
                Some((*rect, *clip, thumb.as_ref().unwrap()))
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        clip,
        rect.intersect(frame.bounds)
            .intersect(Rect::new(0., 0., 110., 70.))
    );
    assert!(thumb.images.iter().all(Option::is_none));
    bindings.widget_mut("description").enabled = Some(false);
    let disabled = document
        .window("description")
        .unwrap()
        .layout(Rect::new(0., 0., 120., 80.), &bindings);
    assert!(disabled.hit_test([4., 4.]).is_none());
    assert!(disabled.hit_test([108., 4.]).is_none());
}

#[test]
fn resolves_anchor_defaults_bindings_and_hit_order() {
    let document = UiDocument::from_xml(XML).unwrap();
    let mut bindings = UiBindings::default();
    bindings.widget_mut("Go").hovered = true;
    bindings.widget_mut("Go").text = Some("Enter".into());
    let frame = document
        .window("window")
        .unwrap()
        .layout(Rect::new(0., 0., 640., 480.), &bindings);
    let hit = frame.hit_test([31., 41.]).unwrap();
    assert_eq!(hit.item, "button");
    assert_eq!(hit.rect, Rect::new(30., 40., 80., 20.));
    assert!(
        frame
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Image { source, .. } if source.x == 44.))
    );
    assert!(
        frame
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Enter"))
    );
    assert_eq!(
        frame
            .commands
            .iter()
            .filter(|command| matches!(command, DrawCommand::Image { .. }))
            .count(),
        1
    );
}

#[test]
fn gauge_clips_without_rescaling_texture_and_animation_uses_durations() {
    let document = UiDocument::from_xml(XML).unwrap();
    let mut bindings = UiBindings {
        time_ms: 150,
        ..Default::default()
    };
    bindings.eq_gauges.insert("1".into(), 0.25);
    let frame = document
        .window("window")
        .unwrap()
        .layout(Rect::new(0., 0., 640., 480.), &bindings);
    let (rect, clip, source) = frame
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Image {
                rect,
                clip,
                source,
                tint,
                ..
            } if tint[0] == 240 => Some((*rect, *clip, *source)),
            _ => None,
        })
        .unwrap();
    assert_eq!(rect, Rect::new(30., 70., 80., 10.));
    assert_eq!(clip, Rect::new(30., 70., 20., 10.));
    assert_eq!(source, Rect::new(24., 8., 20., 10.));
    assert_eq!(
        document.animations["normal"]
            .frame(300, None)
            .unwrap()
            .source
            .x,
        4.
    );
    assert_eq!(
        document.animations["normal"]
            .frame(0, Some(1))
            .unwrap()
            .source
            .x,
        24.
    );
}

#[test]
fn clipping_disabled_controls_and_window_position_are_consistent() {
    let document = UiDocument::from_xml(XML).unwrap();
    let mut bindings = UiBindings::default();
    bindings.widget_mut("window").rect = Some(Rect::new(-10., -10., 100., 80.));
    bindings.widget_mut("Go").enabled = Some(false);
    let frame = document
        .window("window")
        .unwrap()
        .layout(Rect::new(0., 0., 50., 50.), &bindings);
    assert_eq!(frame.hit_test([5., 5.]).unwrap().item, "window");
    for command in frame.commands {
        let clip = match command {
            DrawCommand::Line { clip, .. }
            | DrawCommand::TextLog { clip, .. }
            | DrawCommand::TextArea { clip, .. }
            | DrawCommand::Fill { clip, .. }
            | DrawCommand::Image { clip, .. }
            | DrawCommand::Text { clip, .. } => clip,
        };
        assert!(clip.x >= 0. && clip.y >= 0. && clip.right() <= 50. && clip.bottom() <= 50.);
    }
}

#[test]
fn detects_missing_and_recursive_pieces() {
    let document = UiDocument::from_xml("<XML><Screen item='a'><Pieces>b</Pieces></Screen><Screen item='b'><Pieces>a</Pieces></Screen></XML>").unwrap();
    assert!(matches!(document.window("a"), Err(UiError::Cycle(_))));
    let document =
        UiDocument::from_xml("<XML><Screen item='a'><Pieces>missing</Pieces></Screen></XML>")
            .unwrap();
    assert!(matches!(document.window("a"), Err(UiError::Missing(_))));
}

#[test]
fn loads_includes_in_order_case_insensitively_and_rejects_cycles() {
    let directory = std::env::temp_dir().join(format!("openeq-ui-test-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("Child.XML"),
        "<XML><Label item='label'><Text>included</Text></Label></XML>",
    )
    .unwrap();
    std::fs::write(directory.join("root.xml"), "<XML><Composite><Include>child.xml</Include></Composite><Label item='label'><Text>overridden</Text></Label></XML>").unwrap();
    let document = UiDocument::load(&directory, "root.xml").unwrap();
    assert_eq!(document.source_files.len(), 2);
    assert_eq!(
        document.definition("label").unwrap().value("Text"),
        Some("overridden")
    );
    std::fs::write(
        directory.join("root.xml"),
        "<XML><Composite><Include>root.xml</Include></Composite></XML>",
    )
    .unwrap();
    assert!(matches!(
        UiDocument::load(&directory, "root.xml"),
        Err(UiError::Cycle(_))
    ));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn treats_markup_as_data_and_masks_explicit_password_fields() {
    let document = UiDocument::from_xml("<XML><Editbox item='password'><Size><CX>100</CX><CY>20</CY></Size><Text>ignored</Text></Editbox></XML>").unwrap();
    let mut bindings = UiBindings::default();
    bindings.widget_mut("password").text = Some("sëcret".into());
    bindings.widget_mut("password").password = true;
    let frame = document
        .window("password")
        .unwrap()
        .layout(Rect::new(0., 0., 100., 100.), &bindings);
    assert!(matches!(&frame.commands[0], DrawCommand::Text { text, .. } if text == "••••••"));
    assert!(UiDocument::from_xml("<!DOCTYPE XML [<!ENTITY secret SYSTEM 'file:///tmp/nope'>]><XML><Label item='x'><Text>&secret;</Text></Label></XML>").is_err());
}

#[test]
fn resolves_type_qualified_piece_references() {
    let document = UiDocument::from_xml("<XML><Label item='label'><Size><CX>80</CX><CY>20</CY></Size><Text>typed reference</Text></Label><Screen item='window'><Size><CX>100</CX><CY>40</CY></Size><Pieces>Label:label</Pieces></Screen></XML>").unwrap();
    let frame = document
        .window("Screen:window")
        .unwrap()
        .layout(Rect::new(0., 0., 100., 40.), &UiBindings::default());
    assert!(
        frame.commands.iter().any(
            |command| matches!(command, DrawCommand::Text {text,..} if text=="typed reference")
        )
    );
    assert!(document.definition("Button:label").is_none());
}

#[test]
fn grid_cells_preserve_atlas_boundaries_and_do_not_animate() {
    let document = UiDocument::from_xml(r#"<XML><Ui2DAnimation item="icons"><Grid>true</Grid><CellWidth>40</CellWidth><CellHeight>40</CellHeight><Cycle>true</Cycle><Frames><Texture>first.dds</Texture><Location><X>4</X><Y>8</Y></Location><Size><CX>256</CX><CY>256</CY></Size></Frames><Frames><Texture>second.dds</Texture><Location><X>0</X><Y>0</Y></Location><Size><CX>80</CX><CY>40</CY></Size></Frames></Ui2DAnimation></XML>"#).unwrap();
    let icons = &document.animations["icons"];
    assert_eq!(icons.frames.len(), 38);
    assert_eq!(
        icons.frame(90000, None).unwrap().source,
        Rect::new(4., 8., 40., 40.)
    );
    assert_eq!(
        icons.frame(0, Some(5)).unwrap().source,
        Rect::new(204., 8., 40., 40.)
    );
    assert_eq!(
        icons.frame(0, Some(6)).unwrap().source,
        Rect::new(4., 48., 40., 40.)
    );
    assert_eq!(
        icons.frame(0, Some(35)).unwrap().source,
        Rect::new(204., 208., 40., 40.)
    );
    assert_eq!(icons.frame(0, Some(36)).unwrap().texture, "second.dds");
    assert_eq!(
        icons.frame(0, Some(37)).unwrap().source,
        Rect::new(40., 0., 40., 40.)
    );
    assert!(icons.frame(0, Some(38)).is_none());
}

#[test]
fn vertical_grids_enumerate_columns_before_rows() {
    let document = UiDocument::from_xml(r#"<XML><Ui2DAnimation item="icons"><Grid>true</Grid><Vertical>true</Vertical><CellWidth>40</CellWidth><CellHeight>40</CellHeight><Frames><Texture>icons.dds</Texture><Size><CX>256</CX><CY>256</CY></Size></Frames></Ui2DAnimation></XML>"#).unwrap();
    let icons = &document.animations["icons"];
    assert_eq!(icons.frames.len(), 36);
    assert_eq!(icons.frames[1].source, Rect::new(0., 40., 40., 40.));
    assert_eq!(icons.frames[6].source, Rect::new(40., 0., 40., 40.));
    assert_eq!(icons.frames[29].source, Rect::new(160., 200., 40., 40.));
    assert_eq!(icons.frames[34].source, Rect::new(200., 160., 40., 40.));
}
