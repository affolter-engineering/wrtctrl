use crate::app::{read_generic_config, read_toml_config, write_generic_config, write_toml_config, ConfigRefreshBus, DeviceConfig, GenericConfig};
use leptos::prelude::*;
use leptos::task::spawn_local;


#[component]
pub fn ConfigModal(show_config: RwSignal<bool>) -> impl IntoView {
    let generic_config = LocalResource::new(|| async move { read_generic_config().await });
    let polling_interval = RwSignal::new(String::new());
    let saving = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let config_refresh_bus = use_context::<ConfigRefreshBus>();

    Effect::new(move |_| {
        if let Some(Ok(config)) = generic_config.get() {
            polling_interval.set(config.polling_interval.to_string());
        }
    });

    let on_save = {
        move |_| {
            let polling_interval_raw = polling_interval.get();
            let parsed = polling_interval_raw.trim().parse::<u64>();
            let parsed = match parsed {
                Ok(value) if value > 0 => value,
                _ => {
                    error.set("Polling interval must be a positive number.".to_string());
                    return;
                }
            };

            saving.set(true);
            error.set(String::new());
            spawn_local(async move {
                let result = write_generic_config(GenericConfig {
                    polling_interval: parsed,
                })
                .await;
                match result {
                    Ok(_) => {
                        saving.set(false);
                        if let Some(bus) = config_refresh_bus {
                            bus.tick.update(|t| *t += 1);
                        }
                        show_config.set(false);
                    }
                    Err(e) => {
                        saving.set(false);
                        error.set(format!("Failed to save configuration: {e}"));
                    }
                }
            });
        }
    };

    view! {
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-gray-900/40 px-4">
            <div class="w-full max-w-lg rounded-lg border border-gray-200 bg-white p-6 text-left shadow-2xl">
                <h2 class="text-lg font-semibold text-black">"Generic Configuration"</h2>
                <p class="mt-1 text-xs text-gray-600">"Only generic settings can be edited here."</p>

                <div class="mt-4 space-y-4">
                    <div>
                        <label class="block text-sm font-medium text-black">"Polling Interval (seconds)"</label>
                        <input
                            type="number"
                            min="1"
                            step="1"
                            class="mt-1 w-full rounded border border-gray-300 px-3 py-2 text-black"
                            prop:value=move || polling_interval.get()
                            on:input=move |ev| polling_interval.set(event_target_value(&ev))
                        />
                    </div>

                    {move || {
                        let message = error.get();
                        if message.is_empty() {
                            ().into_any()
                        } else {
                            view! { <p class="text-xs text-red-600">{message}</p> }.into_any()
                        }
                    }}
                </div>

                <div class="mt-6 flex justify-end gap-3">
                    <button
                        class="rounded-md bg-gray-100 px-3 py-2 text-sm font-medium text-black transition hover:bg-gray-200"
                        on:click=move |_| show_config.set(false)
                        type="button"
                        disabled=saving.get()
                    >
                        "Cancel"
                    </button>
                    <button
                        class="rounded-md bg-indigo-100 px-3 py-2 text-sm font-medium text-black transition hover:bg-indigo-200 disabled:opacity-50"
                        on:click=on_save
                        type="button"
                        disabled=move || saving.get()
                    >
                        {move || if saving.get() { "Saving..." } else { "Save" }}
                    </button>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn DeviceDetailsModal(
    device_name: Option<String>,
    show_details: RwSignal<Option<String>>,
) -> impl IntoView {
    let config_data = LocalResource::new(|| async move { read_toml_config().await });
    let saving = RwSignal::new(false);
    let config_refresh_bus = use_context::<ConfigRefreshBus>();
    let name = RwSignal::new(device_name.clone().unwrap_or_default());
    let comment = RwSignal::new(String::new());
    let device_type = RwSignal::new(String::new());
    let ip_address = RwSignal::new(String::new());
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());

    let device_name_for_header = device_name.clone();
    let original_name = device_name.clone();
    let device_name_for_button = device_name.clone();

    Effect::new(move |_| {
        if let (Some(Ok(config)), Some(dev_name)) = (config_data.get(), device_name.clone()) {
            if let Some(devices) = config.devices.as_ref() {
                if let Some(device) = devices.iter().find(|d| d.name == dev_name) {
                    name.set(device.name.clone());
                    comment.set(device.comment.clone().unwrap_or_default());
                    device_type.set(device.device_type.clone().unwrap_or_else(|| "openwrt".to_string()));
                    ip_address.set(device.ip_address.clone().unwrap_or_default());
                    username.set(device.username.clone().unwrap_or_default());
                    password.set(device.password.clone().unwrap_or_default());
                }
            }
        }
    });

    let on_save = {
        let name = name.clone();
        let comment = comment.clone();
        let device_type = device_type.clone();
        let ip_address = ip_address.clone();
        let username = username.clone();
        let password = password.clone();
        let show_details = show_details.clone();
        move |_| {
            saving.set(true);
            let new_name = name.get();
            let comment = comment.get();
            let device_type = device_type.get();
            let ip_address = ip_address.get();
            let username = username.get();
            let password = password.get();
            let original = original_name.clone();
            spawn_local(async move {
                let mut config = match read_toml_config().await {
                    Ok(cfg) => cfg,
                    Err(_) => { saving.set(false); return; }
                };
                let devices = config.devices.get_or_insert_with(Vec::new);
                if let Some(orig) = original.as_deref() {
                    if let Some(existing) = devices.iter_mut().find(|d| d.name == orig) {
                        existing.name = new_name.clone();
                        existing.comment = if comment.is_empty() { None } else { Some(comment.clone()) };
                        existing.device_type = if device_type.is_empty() { None } else { Some(device_type.clone()) };
                        existing.ip_address = if ip_address.is_empty() { None } else { Some(ip_address.clone()) };
                        existing.username = if username.is_empty() { None } else { Some(username.clone()) };
                        existing.password = if password.is_empty() { None } else { Some(password.clone()) };
                    }
                } else {
                    devices.push(DeviceConfig {
                        name: new_name.clone(),
                        comment: if comment.is_empty() { None } else { Some(comment.clone()) },
                        device_type: if device_type.is_empty() { None } else { Some(device_type.clone()) },
                        ip_address: if ip_address.is_empty() { None } else { Some(ip_address.clone()) },
                        username: if username.is_empty() { None } else { Some(username.clone()) },
                        password: if password.is_empty() { None } else { Some(password.clone()) },
                    });
                }
                let _ = write_toml_config(config).await;
                saving.set(false);
                if let Some(bus) = config_refresh_bus {
                    bus.tick.update(|t| *t += 1);
                }
                show_details.set(None);
            });
        }
    };

    view! {
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-gray-900/40 px-4">
            <div class="w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 text-left shadow-2xl">
                <h2 class="text-lg font-semibold text-black">{if device_name_for_header.is_some() { "Edit Device" } else { "Add Device" }}</h2>
                <div class="mt-4 space-y-4">
                    <div>
                        <label class="block text-sm font-medium text-black">"Device Name"</label>
                        <input
                            type="text"
                            class="mt-1 w-full rounded border border-gray-300 px-3 py-2 text-black"
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                    </div>
                    <div>
                        <label class="block text-sm font-medium text-black">"Type"</label>
                        <select
                            class="mt-1 w-full rounded border border-gray-300 px-3 py-2 text-black"
                            prop:value=move || device_type.get()
                            on:change=move |ev| device_type.set(event_target_value(&ev))
                        >
                            <option value="openwrt">"openwrt"</option>
                            <option value="teltonika">"teltonika"</option>
                        </select>
                    </div>
                    <div>
                        <label class="block text-sm font-medium text-black">"IP Address"</label>
                        <input
                            type="text"
                            class="mt-1 w-full rounded border border-gray-300 px-3 py-2 text-black"
                            prop:value=move || ip_address.get()
                            on:input=move |ev| ip_address.set(event_target_value(&ev))
                        />
                    </div>
                    <div>
                        <label class="block text-sm font-medium text-black">"Username"</label>
                        <input
                            type="text"
                            class="mt-1 w-full rounded border border-gray-300 px-3 py-2 text-black"
                            prop:value=move || username.get()
                            on:input=move |ev| username.set(event_target_value(&ev))
                        />
                    </div>
                    <div>
                        <label class="block text-sm font-medium text-black">"Password"</label>
                        <input
                            type="text"
                            class="mt-1 w-full rounded border border-gray-300 px-3 py-2 text-black"
                            prop:value=move || password.get()
                            on:input=move |ev| password.set(event_target_value(&ev))
                        />
                    </div>
                    <div>
                        <label class="block text-sm font-medium text-black">"Comment"</label>
                        <input
                            type="text"
                            class="mt-1 w-full rounded border border-gray-300 px-3 py-2 text-black"
                            prop:value=move || comment.get()
                            on:input=move |ev| comment.set(event_target_value(&ev))
                        />
                    </div>
                </div>
                <div class="mt-6 flex justify-end gap-3">
                    <button
                        class="rounded-md bg-gray-100 px-3 py-2 text-sm font-medium text-black transition hover:bg-gray-200"
                        on:click=move |_| show_details.set(None)
                        type="button"
                        disabled=saving.get()
                    >
                        "Cancel"
                    </button>
                    <button
                        class="rounded-md bg-indigo-100 px-3 py-2 text-sm font-medium text-black transition hover:bg-indigo-200 disabled:opacity-50"
                        on:click=on_save
                        type="button"
                        disabled=move || saving.get() || name.get().is_empty()
                    >
                        {move || if saving.get() { "Saving..." } else { if device_name_for_button.is_some() { "Save" } else { "Add" } }}
                    </button>
                </div>
            </div>
        </div>
    }
}
