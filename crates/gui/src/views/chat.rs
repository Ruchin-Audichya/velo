//! The connected view: sidebar, conversation, and whatever floats over them.

use gpui::AnimationExt as _;
use gpui::{
    AnyElement, App, AppContext, Axis, Context, DragMoveEvent, Entity, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::FluentBuilder as _,
};
use gpui_base::{HandleEdge, InteractiveElementExt as _, resize_handle};
use gpui_component::ActiveTheme as _;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Icon, IconName};

use crate::app::{Destination, MessageListCache, OfflineRecovery, WhatsAppApp};
use crate::components::{
    ChatListProps, EmptyState, InputAreaView, PluginContext, ProductIcon, StatusListProps,
    StatusSelection, StatusViewProps, ViewerProps, parts, plugin_ui, render_call_card,
    render_chat_header, render_chat_list, render_conversation_search, render_media_viewer,
    render_message_list, render_nav_rail, render_status_list, render_status_view,
};
use crate::responsive::ResponsiveLayout;
use crate::theme::Metrics;

use velo_core::{CallState, Chat};

struct ResizeSidebar;

impl Render for ResizeSidebar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub fn render_connected_view(
    app: &mut WhatsAppApp,
    calls: &CallState,
    window: &mut Window,
    cx: &mut Context<WhatsAppApp>,
) -> impl IntoElement + use<> {
    app.ensure_file_drop(cx);
    app.ensure_input_area(window, cx);
    app.ensure_chat_search_input(window, cx);
    // Before anything reads a plugin's tree: a field the plugin added this
    // frame needs somewhere to hold what is typed into it, and only the
    // render pass knows which widgets are on screen.
    app.sync_plugin_fields(window, cx);

    let layout = app.responsive_layout(window, cx);
    let entity = cx.entity().clone();
    // Built here, where the app is still reachable: a header component takes
    // neither the app nor a `Context`, and drawing a plugin's widget needs
    // both. Owned elements, so nothing below is borrowing the app.
    let plugin_ctx = PluginContext {
        entity: entity.clone(),
        metrics: *layout.metrics(),
    };
    let mut plugin_actions = plugin_ui::slot(
        app.plugins(cx),
        velo_core::PluginSlot::ChatHeader,
        app,
        &plugin_ctx,
        cx,
    );
    let show_details_toggle = layout.can_show_details() && app.destination().has_conversations();
    if show_details_toggle {
        let entity = entity.clone();
        plugin_actions.push(
            Button::new("shared-files-toggle")
                .ghost()
                .icon(ProductIcon::FileText)
                .tooltip("Shared files")
                .on_click(move |_, _, cx| {
                    entity.update(cx, |app, cx| {
                        app.shared_files_open = !app.shared_files_open;
                        if app.shared_files_open {
                            app.contact_info_open = false;
                        }
                        cx.notify();
                    })
                })
                .into_any_element(),
        );
    }
    let selected_jid = app.selected_destination_jid();
    // Cloned rather than borrowed: building the timeline's rows below needs
    // the app mutably, and these are handles — a clone is a refcount.
    let chat_list_scroll = app.chat_list_scroll().clone();
    let chat_list_focus = app.chat_list_focus().clone();
    let chat_search_input = app.chat_search_input(cx).cloned();

    let message_list = app.message_list().clone();
    let away_from_latest = app.away_from_latest;
    let input_area = app.input_area();
    // The composer draws itself into the slot the layout gave it, and only
    // this side knows what that is: left to its own defaults it used the
    // desktop height and icon size on a phone.
    if let Some(input) = &input_area {
        input.update(cx, |view, cx| {
            view.set_layout(layout.input_area_height(), layout.min_touch_target(), cx);
        });
    }
    // What this frame draws as the conversation, which is what decides
    // whether an arriving message has been seen. On a phone the chat list and
    // the conversation are the same slot, Status replaces both, and the
    // fullscreen viewer covers whatever is underneath it — a picture at the
    // size of the window is a mode, so the timeline behind it is no more
    // visible than a chat on another screen.
    let visible = (app.destination().has_conversations()
        && layout.show_chat_area()
        && !app.paste_preview_showing()
        && app.media_viewer(cx).is_none())
    .then(|| selected_jid.clone())
    .flatten();
    app.note_visible_conversation(visible, window, cx);
    // Against what this frame is about to draw, and before anything reads the
    // chat list: a chat a complete load said was gone is kept only while it is
    // on screen, and this is where looking away is noticed.
    app.prune_departed_chats(cx);

    let viewer_focus = app.viewer_focus(cx).clone();
    let media = app.media_cache();

    let chat_list_cache = app.get_chat_list_cache(cx);
    let list_selected_jid = app.chat_list_selection_jid(&chat_list_cache);
    let list_props = ChatListProps {
        cache: chat_list_cache,
        destination: app.destination(),
        loading: app.chat_list_loading(cx),
        selected_jid: list_selected_jid,
        filter: app.chat_filter(),
        unread_count: app.unread_chat_count(),
        is_searching: app.is_searching(cx),
        search_input: chat_search_input.as_ref(),
        media: media.clone(),
        message_hits: app.global_search.hits.clone(),
        message_hits_loading: app.global_search.loading,
    };
    // A filter that matches none of the chats this window holds is not the
    // same as one that matches nothing: the rest of the list has not been
    // asked for yet, and the row that would answer a search — or the only
    // unread chat — can be on a page nobody has fetched. An empty list is at
    // its end by definition, so it asks like any other list that is; the
    // paging state is what stops it asking twice, and `Done` is what ends it.
    if app.destination().has_conversations() && (app.chat_list_is_empty(cx) || app.is_searching(cx))
    {
        app.want_more_chats(cx);
    }

    // The conversation is named rather than held, so nothing here borrows
    // `app` across the calls below that need it mutably. It used to be cloned
    // for exactly that, which meant copying every message in the conversation
    // once per frame for readers that only look at the timeline cache.
    let open_chat = selected_jid.clone().filter(|jid| app.has_chat(jid));
    let typing = open_chat.as_ref().and_then(|jid| app.typing_in(jid));
    let availability = open_chat
        .as_ref()
        .and_then(|jid| app.availability_of(jid))
        .cloned();
    let message_cache = open_chat
        .as_ref()
        .map(|jid| app.get_message_list_cache(jid, typing.clone(), layout, cx));
    // A search hit picked before its chat's messages had loaded: the cache
    // just built is what the jump scrolls through, so this is the first
    // moment it can land.
    if message_cache.is_some() && app.global_search.pending_jump.is_some() {
        app.retry_pending_search_jump(cx);
    }
    // A call in a chat other than the one on screen is what the return banner
    // is for; a call in *this* chat is already obvious from the card.
    // Body observes state changes separately from video-frame notifications.
    // Reading the Calls entity here would make GPUI Fast invalidate the body
    // for every picture even when this banner has not changed.
    let banner = calls
        .active()
        .filter(|call| open_chat.as_deref().is_none_or(|jid| jid != call.peer_jid))
        .map(|call| (call.peer_name.clone(), call.elapsed_label()));
    // Rendered as an element here rather than passed down as state: the
    // search belongs to the conversation pane, and only this level has both
    // the app and the entity to drive it from.
    let search_bar = app.conversation_search(cx).map(|search| {
        crate::motion::fade_in(
            div().w_full().child(render_conversation_search(
                search,
                app.conversation_search_input(cx),
                entity.clone(),
                *layout.metrics(),
                cx,
            )),
            "conversation-search-in",
        )
        .into_any_element()
    });
    // The picture, when one is open. Above the conversation and below the
    // call card: a photo can wait, an incoming call cannot.
    let viewer = app.media_viewer(cx).and_then(|viewer| {
        let message = app.media_viewer_message(cx)?.clone();
        let media = message.media.as_ref()?;
        let image = (!media.data.is_empty() && !media.data_is_preview)
            .then(|| app.get_decoded_image(&message.id, media, cx))
            .flatten();
        // The thumbnail, drawn large while the full picture downloads.
        let preview = (image.is_none() && media.data_is_preview && !media.data.is_empty())
            .then(|| app.get_decoded_image(&message.id, media, cx))
            .flatten();
        let frame = app.video_current_frame(&message.id);
        let author = if message.is_from_me {
            SharedString::from(crate::tr!(You))
        } else {
            SharedString::from(app.author_label(open_chat.as_deref(), &message))
        };
        let is_loading =
            app.awaiting_cached_media(&viewer.jid, &message) || app.is_downloading(&message.id);
        Some(
            render_media_viewer(
                viewer,
                ViewerProps {
                    message,
                    image,
                    preview,
                    frame,
                    is_loading,
                    author,
                },
                entity.clone(),
                &viewer_focus,
                *layout.metrics(),
                cx,
            )
            .into_any_element(),
        )
    });
    // Status, when that is where the window is. Resolved here for the same
    // reason the viewer is: the picture is the app's to decode, and this is
    // the level that still has it.
    let destination = app.destination();
    // Before the feed is read, because the reader is anchored to an update
    // and a rebuild can have moved it. Only where the reader is on screen:
    // the pane keeps its author while the window is elsewhere, and deriving
    // the feed for a frame that does not draw it is work for nothing. See
    // `reconcile_status_pane`.
    if destination == Destination::Status {
        app.ensure_status_page(cx);
        app.reconcile_status_pane(cx);
    }
    let status_feed = (destination == Destination::Status).then(|| app.status_feed());
    let status_view = status_feed.as_ref().and_then(|feed| {
        let selected = app.status_pane().author()?;
        let author = feed.author(selected)?;
        let at = app.status_pane().index_in(author.count());
        let message = feed.updates_of(author).nth(at)?.clone();
        // `and_then`, not `map`: a video update's bytes are its own MP4 once
        // it has been fetched, and there is no still in them to decode. The
        // frame below is what such an update draws.
        let image = message
            .media
            .as_ref()
            .filter(|media| !media.data.is_empty())
            .and_then(|media| app.get_decoded_image(&message.id, media, cx));
        let frame = app.video_current_frame(&message.id);
        // A video says it is loading in its *player*, not in the download
        // table: `start_video_download` records progress there and nowhere
        // else, so asking only the table left a video status reading "cannot
        // be shown here" for the whole download.
        let is_loading_update = image.is_none()
            && frame.is_none()
            && (app.is_downloading(&message.id)
                || app
                    .video_player_state(&message.id)
                    .is_some_and(|state| state.is_loading()));
        Some(StatusViewProps {
            author_jid: author.jid.clone(),
            author_name: author.name.clone().into(),
            message,
            image,
            frame,
            index: at,
            count: author.count(),
            is_loading: is_loading_update,
        })
    });
    app.note_media_surface(cx);
    let status_list = status_feed.map(|feed| StatusListProps {
        pictures: feed
            .authors()
            .iter()
            .filter_map(|author| {
                let key = app.chat_named(&author.jid)?.avatar_cache_key.clone()?;
                Some((author.jid.clone(), key))
            })
            .collect(),
        feed,
        can_load_more: app.status_can_load_more(cx),
        selected: StatusSelection::of(app.status_pane().author()),
        scroll: app.status_list_scroll().clone(),
    });
    // Read-only: the user stopped waiting for a connection. The composer is
    // replaced rather than disabled in place, because the interesting part is
    // the way out, not the field.
    let offline_recovery = app.offline_recovery();
    let is_offline = offline_recovery.is_some();
    // "(You)" on the conversation with your own number, as on its list row.
    let is_own_number = open_chat
        .as_deref()
        .is_some_and(|jid| app.is_own_number(jid));
    let can_send = app.can_send();

    // Which surfaces this frame gives the keyboard somewhere to land. Only
    // the view drawing them can say: the composer outlives the conversation
    // that made it, the offline strip takes its place, and on a phone the
    // list and the conversation are one slot. See `sync_overlay_focus`.
    app.note_keyboard_surfaces(crate::app::KeyboardSurfaces {
        chat_list: destination.has_conversations() && layout.show_sidebar(),
        composer: destination.has_conversations()
            && layout.show_chat_area()
            && open_chat.is_some()
            && !is_offline,
        viewer: viewer.is_some(),
        paste_preview: false,
        message_edit: false,
        message_delete: false,
        // The card floats above this view rather than inside it, so the root
        // is what knows whether one was drawn. See `WhatsAppApp::render`.
        call_card: false,
    });

    let rail = render_rail(app, entity.clone(), layout, cx);

    // Borrowed last, once nothing else needs `app` mutably: the header and
    // the empty state are the only readers of the chat itself, and everything
    // the timeline draws has already been gathered.
    let selected_chat = open_chat.as_deref().and_then(|jid| app.chat_named(jid));
    // Who is in it, where it is a group and the daemon has said. Asked for
    // when the conversation was opened; until the answer lands the header
    // draws the name alone, as it did before there was anything to draw.
    let members = open_chat.as_deref().and_then(|jid| app.group_members(jid));
    // Why the message box would only fail here, when the daemon has said.
    let posting_blocked = members.and_then(|roster| roster.posting_blocked);
    // The two panes, whichever destination they belong to.
    let panes = div()
        .flex()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .bg(cx.theme().sidebar)
        .when(layout.show_sidebar(), |el| {
            let sidebar = match status_list {
                Some(props) => {
                    render_status_list(props, entity.clone(), layout, cx).into_any_element()
                }
                None => render_chat_list(
                    list_props,
                    &chat_list_scroll,
                    &chat_list_focus,
                    entity.clone(),
                    layout,
                    cx,
                )
                .into_any_element(),
            };
            // Switching between Chats, Status, Channels and Communities slides
            // the new list up a few points as it fades in: a fifth of a
            // second, transform and opacity only, so it never costs a frame.
            let sidebar = crate::motion::rise_in(
                div().h_full().child(sidebar),
                gpui::ElementId::Name(format!("pane-{}", destination.label()).into()),
                layout.metrics().space_md(),
            );
            el.child(
                div()
                    .relative()
                    .h_full()
                    .flex_shrink_0()
                    .child(sidebar)
                    .when(!layout.is_mobile(), |sidebar| {
                        let drag_entity = entity.clone();
                        let reset_entity = entity.clone();
                        sidebar
                            .on_drag_move(
                                move |event: &DragMoveEvent<ResizeSidebar>, window, cx| {
                                    drag_entity.update(cx, |app, cx| {
                                        let layout = app.responsive_layout(window, cx);
                                        if !layout.is_mobile() {
                                            let width =
                                                event.event.position.x - layout.rail_width();
                                            let wanted = Some(
                                                f32::from(width) / layout.metrics().rem_size(),
                                            );
                                            if app.sidebar_preference_rem != wanted {
                                                app.sidebar_preference_rem = wanted;
                                                cx.notify();
                                            }
                                        }
                                    });
                                },
                            )
                            .child(
                                resize_handle("chat-sidebar-divider", Axis::Horizontal)
                                    .inside(HandleEdge::Trailing)
                                    .on_drag(ResizeSidebar, |_, _, _, cx| cx.new(|_| ResizeSidebar))
                                    .on_double_click(move |_, _, cx| {
                                        reset_entity.update(cx, |app, cx| {
                                            app.sidebar_preference_rem = None;
                                            cx.notify();
                                        });
                                    }),
                            )
                    }),
            )
        })
        .when(layout.show_chat_area(), |el| {
            if destination == Destination::Status {
                el.child(render_status_view(status_view, entity.clone(), layout, cx))
            } else {
                el.child(render_chat_area(
                    ChatAreaProps {
                        selected_chat,
                        message_cache,
                        banner,
                        typing: typing.as_ref(),
                        availability: availability.as_ref(),
                        members,
                        search_bar,
                        input_area,
                        can_send,
                        posting_blocked,
                        offline_recovery,
                        is_own_number,
                        plugin_actions,
                        media: media.clone(),
                        away_from_latest,
                    },
                    &message_list,
                    entity.clone(),
                    layout,
                    cx,
                ))
            }
        });

    let info_open = app.contact_info_open && layout.can_show_details() && layout.show_chat_area();
    let panes = panes.when(info_open, |el| {
        el.children(selected_chat.map(|chat| {
            crate::motion::slide_in_right(
                div()
                    .flex()
                    .h_full()
                    .flex_shrink_0()
                    .child(super::contact_info::render(
                        chat,
                        app.group_members(&chat.jid),
                        &|jid| app.find_chat(jid).and_then(|c| c.avatar_cache_key.clone()),
                        app.is_own_number(&chat.jid),
                        app.is_connected() && !chat.is_group,
                        entity.clone(),
                        media.clone(),
                        *layout.metrics(),
                        cx,
                    )),
                "contact-info-in",
                layout.metrics().space_xxl(),
            )
        }))
    });
    let panes = panes.when(
        !info_open && show_details_toggle && app.shared_files_open && layout.show_chat_area(),
        |el| {
            el.children(selected_chat.map(|chat| {
                // Slides in from the edge it lives on when opened.
                crate::motion::slide_in_right(
                    div()
                        .flex()
                        .h_full()
                        .flex_shrink_0()
                        .child(super::shared_files::render(
                            app,
                            chat,
                            app.shared_files_filter,
                            app.can_load_older_shared_files(&chat.jid, cx),
                            entity.clone(),
                            *layout.metrics(),
                            cx,
                        )),
                    "shared-files-in",
                    layout.metrics().space_xxl(),
                )
            }))
        },
    );

    // A strip down the side where there is width for one, a bar across the
    // foot where there is not — and on a phone only while the list is on
    // screen, because a conversation there is the whole window.
    let shell = if layout.is_mobile() {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(panes)
            .when(layout.show_sidebar(), |el| el.child(rail))
    } else {
        div().flex().size_full().child(rail).child(panes)
    };

    div().relative().size_full().child(shell).children(viewer)
}

/// The floating call card, for whichever screen the window is on.
///
/// Drawn by the root rather than by the conversation view, because a call is
/// not a thing about conversations: one arriving while Settings is open used
/// to ring at the far end with no card, no Accept and no Decline anywhere in
/// the window. It floats and takes no input of its own, so the screen
/// underneath stays usable for the whole call.
/// The navigation strip, with its unread counts and the account photo.
///
/// Its own function so the screens that replace the chat panes — Calls,
/// Files — keep it: a full-window screen with no way back but a text link
/// is a dead end, which the desktop messengers never are.
pub fn render_rail(
    app: &WhatsAppApp,
    entity: Entity<WhatsAppApp>,
    layout: crate::responsive::ResponsiveLayout,
    cx: &App,
) -> impl IntoElement + use<> {
    let screen = if app.calls_open {
        Some(crate::components::RailScreen::Calls)
    } else if app.files_open {
        Some(crate::components::RailScreen::Files)
    } else {
        None
    };
    render_nav_rail(
        app.destination(),
        screen,
        [
            app.unread_chat_count(),
            app.status_unseen(),
            app.unread_destination_count(Destination::Channels),
            app.unread_destination_count(Destination::Communities),
        ],
        app.account_summary(),
        entity,
        layout,
        cx,
    )
}

pub fn render_call_overlay(
    app: &mut WhatsAppApp,
    window: &mut Window,
    cx: &mut Context<WhatsAppApp>,
) -> Option<AnyElement> {
    let layout = app.responsive_layout(window, cx);
    let call_focus = app.call_focus().clone();
    render_call_card(
        app.call_state(cx),
        app.call_card(cx),
        cx.entity().clone(),
        &call_focus,
        layout,
        app,
        cx,
    )
}

/// Everything the conversation pane draws, gathered by the caller so this
/// stays a function of one frame's state.
struct ChatAreaProps<'a> {
    selected_chat: Option<&'a Chat>,
    message_cache: Option<MessageListCache>,
    banner: Option<(String, String)>,
    typing: Option<&'a velo_core::TypingSummary>,
    availability: Option<&'a velo_core::Availability>,
    /// Who is in the open conversation, when it is a group the daemon has
    /// answered for.
    members: Option<&'a velo_core::GroupRoster>,
    search_bar: Option<gpui::AnyElement>,
    input_area: Option<Entity<InputAreaView>>,
    /// Whether anything can be sent from here at all.
    can_send: bool,
    /// Why this account may not post in the open group or channel: the
    /// message box gives way to a line saying so, as on the phone.
    posting_blocked: Option<velo_core::PostingBlock>,
    offline_recovery: Option<OfflineRecovery>,
    /// Whether the open conversation is with this account's own number.
    is_own_number: bool,
    /// What plugins want in the conversation's header, already rendered.
    ///
    /// Built here rather than inside the header, because drawing a widget
    /// needs the app and the header component takes neither it nor a
    /// `Context`. Empty when no plugin drew there, which is the ordinary
    /// case.
    plugin_actions: Vec<gpui::AnyElement>,
    media: Option<std::sync::Arc<dyn crate::session::MediaCache>>,
    /// Scrolled up from the newest message: show the jump-back button.
    away_from_latest: bool,
}

fn render_chat_area(
    props: ChatAreaProps<'_>,
    message_list: &gpui::ListState,
    entity: Entity<WhatsAppApp>,
    layout: ResponsiveLayout,
    cx: &App,
) -> impl IntoElement + use<> {
    let ChatAreaProps {
        selected_chat,
        message_cache,
        banner,
        typing,
        availability,
        members,
        search_bar,
        input_area,
        can_send,
        posting_blocked,
        offline_recovery,
        is_own_number,
        plugin_actions,
        away_from_latest,
        media,
    } = props;
    let metrics = *layout.metrics();
    let base = if layout.is_mobile() {
        div().w_full()
    } else {
        div().flex_1().min_w_0()
    };

    base.flex()
        .flex_col()
        .h_full()
        .bg(cx.theme().background)
        .map(|el| match selected_chat {
            // Centred by the same container the startup screens use, which
            // is also the one that does not clip what it centres: an empty
            // state is short, but not shorter than a pane on a handheld.
            None => el.child(crate::motion::fade_in(
                div().size_full().child(
                    super::centered_view("empty-conversation", metrics.space_xxxl())
                        .surface(cx.theme().background)
                        .child(
                            EmptyState::new(crate::tr!(PickAConversation))
                                .icon(ProductIcon::VeloMark)
                                .brand()
                                .description(crate::tr!(
                                    ChooseAChatOnTheLeftOrSearchForOneByNameOrMessage
                                ))
                                .shortcut(
                                    if cfg!(target_os = "macos") {
                                        "⌘K"
                                    } else {
                                        "Ctrl K"
                                    },
                                    crate::tr!(Search),
                                )
                                .shortcut("↑ ↓", crate::tr!(MoveBetweenChats)),
                        )
                        .child(render_why_velo(metrics, cx)),
                ),
                "empty-conversation-in",
            )),
            Some(chat) => {
                let is_group = chat.is_group;
                el.child(render_chat_header(
                    chat,
                    typing,
                    availability,
                    members,
                    is_own_number,
                    can_send,
                    entity.clone(),
                    plugin_actions,
                    layout,
                    media,
                    cx,
                ))
                .children(search_bar)
                .children(banner.map(|(name, elapsed)| {
                    render_return_banner(name, elapsed, entity.clone(), metrics, cx)
                }))
                .children(message_cache.map(|cache| {
                    // Keyed by the chat, so switching conversations fades the
                    // new one in rather than cutting to it.
                    let jump_entity = entity.clone();
                    crate::motion::fade_in(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .relative()
                            .child(render_message_list(
                                cache,
                                message_list,
                                entity.clone(),
                                SharedString::from(chat.jid.clone()),
                                is_group,
                                is_own_number,
                                layout,
                                cx,
                            ))
                            // Scrolled up: a round button back to the newest
                            // message, where WhatsApp keeps its own.
                            .when(away_from_latest, |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .right(metrics.space_xl())
                                        .bottom(metrics.space_lg())
                                        .child(crate::motion::pop_in(
                                            gpui_component::button::Button::new("jump-to-latest")
                                                .icon(gpui_component::IconName::ChevronDown)
                                                .tooltip("Jump to the latest message")
                                                .rounded_full()
                                                .size(metrics.icon_button() * 1.15)
                                                .shadow_md()
                                                .on_click(move |_, _, cx| {
                                                    jump_entity.update(cx, |app, cx| {
                                                        app.jump_to_latest(cx)
                                                    });
                                                }),
                                            "jump-to-latest-in",
                                        )),
                                )
                            }),
                        gpui::ElementId::Name(format!("chat-in-{}", chat.jid).into()),
                    )
                }))
                .map(|el| {
                    if let Some(recovery) = offline_recovery {
                        el.child(render_offline_strip(entity.clone(), recovery, metrics, cx))
                    } else if let Some(block) = posting_blocked {
                        el.child(render_posting_strip(block, metrics, cx))
                    } else {
                        el.children(input_area)
                    }
                })
                .on_drop({
                    let entity = entity.clone();
                    move |paths: &gpui::ExternalPaths, _window, cx| {
                        entity.update(cx, |app, cx| {
                            app.drop_paths(paths.paths().to_vec(), cx);
                        });
                    }
                })
            }
        })
}

/// Three reasons Velo is worth having, on the screen everyone sees first.
///
/// Each card rises in a beat after the one before, so the row reads left to
/// right instead of appearing as a block.
fn render_why_velo(metrics: Metrics, cx: &App) -> impl IntoElement + use<> {
    let cards = [
        (
            ProductIcon::VeloMark,
            "Light and instant",
            "Native Rust drawn on your GPU. No browser inside, a fraction of the memory.",
        ),
        (
            ProductIcon::Sticker,
            "60 stickers built in",
            "Hinglish, English and meme packs, plus every sticker from your chats.",
        ),
        (
            ProductIcon::MessageSquare,
            "Private by design",
            "End-to-end encrypted like WhatsApp Web. Your chats stay on this PC.",
        ),
    ];
    let mut row = div()
        .mt(metrics.space_xxl())
        .flex()
        .flex_wrap()
        .justify_center()
        .gap(metrics.space_lg());
    for (index, (icon, title, body)) in cards.into_iter().enumerate() {
        let card = div()
            .w(metrics.call_card_width() * 0.62)
            .p(metrics.space_lg())
            .rounded(metrics.radius_xl())
            .bg(cx.theme().secondary)
            .border_1()
            .border_color(cx.theme().border)
            .flex()
            .flex_col()
            .gap(metrics.space_sm())
            .child(
                Icon::new(icon)
                    .size(metrics.icon_button() * 0.8)
                    .text_color(cx.theme().primary),
            )
            .child(
                div()
                    .text_size(metrics.text_body())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child(title),
            )
            .child(
                div()
                    .text_size(metrics.text_small())
                    .text_color(cx.theme().muted_foreground)
                    .child(body),
            );
        row = row.child(
            card.with_animation(
                ("why-velo", index),
                gpui::Animation::new(crate::motion::duration(std::time::Duration::from_millis(
                    260 + index as u64 * 90,
                )))
                .with_easing(gpui::ease_out_quint()),
                {
                    let rise = crate::motion::travel(metrics.space_xl());
                    move |el, t| el.relative().top(rise * (1.0 - t)).opacity(t)
                },
            ),
        );
    }
    row
}

/// What replaces the composer while the app is read-only.
///
/// Not a disabled field: a greyed-out composer says "you cannot type here"
/// and stops. What the reader needs is why, and the way back.
pub(crate) fn render_offline_strip(
    entity: Entity<WhatsAppApp>,
    recovery: OfflineRecovery,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let terminal = matches!(recovery, OfflineRecovery::PairAgain);
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(metrics.space_lg())
        .px(metrics.space_xl())
        .py(metrics.space_lg())
        .bg(cx.theme().secondary)
        .border_t_1()
        .border_color(cx.theme().border)
        .child(
            Icon::new(ProductIcon::WifiOff)
                .size(metrics.icon_small())
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(metrics.text_small())
                .text_color(cx.theme().muted_foreground)
                .child(if terminal {
                    crate::tr!(LoggedOutOnlyMessagesAndMediaStoredLocallyAreAvailable)
                } else {
                    crate::tr!(OfflineYourMessagesStayReadableReconnectToSend)
                }),
        )
        .child(
            Button::new("reconnect")
                .label(if terminal {
                    crate::tr!(SessionOptions)
                } else {
                    crate::tr!(Reconnect)
                })
                .ghost()
                .cursor_pointer()
                .on_click(move |_, _window, cx| {
                    entity.update(cx, |app, cx| app.retry_whatsapp_connection(cx));
                }),
        )
}

/// What replaces the message box where this account may not post.
///
/// WhatsApp's own wording, centred where the box was. Not a disabled field:
/// typing into one that can only fail is what the "not sent, retry" bubbles
/// used to be.
fn render_posting_strip(
    block: velo_core::PostingBlock,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    use velo_core::PostingBlock;
    let line = match block {
        PostingBlock::AdminsOnly => crate::tr!(OnlyAdminsCanSendMessages),
        PostingBlock::NotMember => crate::tr!(NoLongerAGroupMember),
        PostingBlock::ChannelFollower => crate::tr!(OnlyChannelAdminsCanPost),
    };
    div()
        .id("posting-blocked")
        .flex_shrink_0()
        .flex()
        .justify_center()
        .px(metrics.space_xl())
        .py(metrics.space_lg())
        .bg(cx.theme().secondary)
        .border_t_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .text_size(metrics.text_small())
                .text_color(cx.theme().muted_foreground)
                .text_center()
                .child(line),
        )
}

/// "On call · 04:12 · Return to call", under the header of some other chat.
///
/// This is what makes the floating card safe to wander away from: the call is
/// still findable from wherever the user ends up.
fn render_return_banner(
    name: String,
    elapsed: String,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .gap(metrics.space_lg())
        .px(metrics.space_xl())
        .py(metrics.space_md())
        .bg(cx.theme().primary.opacity(0.12))
        .border_b_1()
        .border_color(cx.theme().primary.opacity(0.35))
        .child(
            Icon::new(ProductIcon::Phone)
                .size(metrics.icon_small())
                .text_color(cx.theme().primary),
        )
        .child(
            parts::one_line()
                .flex_1()
                .min_w_0()
                .text_size(metrics.text_small())
                .text_color(cx.theme().foreground)
                .child(crate::tr!(on_call_with, name)),
        )
        .child(
            div()
                .font_features(crate::components::parts::tabular())
                .text_size(metrics.text_meta())
                .text_color(cx.theme().primary)
                .child(elapsed),
        )
        .child(
            Button::new("return-to-call")
                .label(crate::tr!(ReturnToCall))
                .icon(Icon::new(IconName::ArrowRight))
                .ghost()
                .cursor_pointer()
                .on_click(move |_, _window, cx| {
                    entity.update(cx, |app, cx| app.return_to_call(cx));
                }),
        )
}
