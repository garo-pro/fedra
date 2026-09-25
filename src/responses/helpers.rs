use crate::{AppState, mastodon::Status, network::NetworkCommand};

pub(super) fn summarize_api_error(err: &anyhow::Error) -> String {
	for cause in err.chain().skip(1) {
		let mut message = cause.to_string();
		if message.trim().is_empty() {
			continue;
		}
		if message.starts_with("HTTP status")
			&& let Some((head, _)) = message.split_once(" for url")
		{
			message = head.to_string();
		}
		return message;
	}
	err.to_string()
}

pub(super) fn spoken_failure(prefix: &str, err: &anyhow::Error) -> String {
	format!("{prefix}: {}", summarize_api_error(err))
}

/// Re-fetches all open user timelines belonging to the current account so that
/// pinned-post ordering reflects the latest pin state.
pub(super) fn refresh_own_user_timelines(state: &AppState) {
	let Some(current_user_id) = &state.current_user_id else { return };
	let Some(handle) = &state.network_handle else { return };
	let types = state.timeline_manager.open_timeline_types();
	for tt in types {
		if let crate::timeline::TimelineType::User { ref id, .. } = tt
			&& id == current_user_id
		{
			handle.send(NetworkCommand::FetchTimeline {
				timeline_type: tt,
				limit: Some(u32::from(state.config.fetch_limit)),
				max_id: None,
			});
		}
	}
}

fn merge_status_snapshot_by_id(state: &mut AppState, status_id: &str, snapshot: &Status) -> bool {
	let active_index = state.timeline_manager.active_index();
	let mut active_changed = false;
	for (index, timeline) in state.timeline_manager.iter_mut().enumerate() {
		for entry in &mut timeline.entries {
			if let Some(status) = entry.as_status_mut() {
				if status.id == status_id {
					*status = snapshot.clone();
					active_changed |= index == active_index;
				}
				if let Some(ref mut reblog) = status.reblog
					&& reblog.id == status_id
				{
					**reblog = snapshot.clone();
					active_changed |= index == active_index;
				}
			}
		}
	}
	active_changed
}

/// Folds a freshly fetched copy of a post into every timeline that shows it, and returns whether
/// the active timeline was one of them, i.e. whether it needs redrawing. A post always matches
/// the timeline it was just added to, so returning true for any match made every streamed post,
/// even one for a background timeline, re-render the whole active timeline.
pub(super) fn merge_status_snapshot(state: &mut AppState, snapshot: &Status) -> bool {
	let mut updated = merge_status_snapshot_by_id(state, &snapshot.id, snapshot);
	if let Some(reblog) = &snapshot.reblog {
		updated |= merge_status_snapshot_by_id(state, &reblog.id, reblog);
	}
	updated
}
