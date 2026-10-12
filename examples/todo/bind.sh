#!/usr/bin/env bash
# Regenerates todo/gtk/sys.vun from GTK's headers: only the functions this program calls, and the
# types they are written with. Needs gtk4-devel and libclang, and the repository's `vun`.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
vun=${VUN:-vun}
header=$(mktemp --suffix=.h)
trap 'rm -f "$header"' EXIT
echo '#include <gtk/gtk.h>' > "$header"

flags=()
for flag in $(pkg-config --cflags gtk4); do flags+=(--cflag "$flag"); done
# The libraries each header directory is linked from.
libraries=(
    --library gtk-4=/usr/include/gtk-4.0
    --library gobject-2.0=/usr/include/glib-2.0/gobject
    --library gio-2.0=/usr/include/glib-2.0/gio
    --library glib-2.0=/usr/include/glib-2.0/glib
)
# The directories the types the calls use are declared in.
scope=()
for dir in /usr/include/gtk-4.0 /usr/include/glib-2.0 /usr/lib64/glib-2.0/include /usr/include/pango-1.0; do
    scope+=(--scope "$dir")
done
# The enums C holds no other value in: Vun enums rather than integers.
closed=()
for name in GtkOrientation GtkSelectionMode PangoEllipsizeMode; do closed+=(--closed "$name"); done
only=()
for name in \
    g_signal_connect_data g_object_set_data g_object_get_data g_object_unref g_application_run \
    gtk_application_new gtk_application_window_new gtk_window_set_title gtk_window_set_default_size \
    gtk_window_set_child gtk_window_present 'gtk_widget_set_margin_*' gtk_widget_set_hexpand \
    gtk_widget_set_vexpand gtk_widget_set_sensitive gtk_widget_add_css_class \
    gtk_widget_grab_focus gtk_box_new gtk_box_append gtk_scrolled_window_new gtk_scrolled_window_set_child \
    gtk_label_new gtk_label_set_text gtk_label_set_xalign gtk_label_set_ellipsize gtk_entry_new \
    gtk_entry_set_placeholder_text gtk_editable_get_text gtk_editable_set_text gtk_editable_set_position \
    gtk_button_new_with_label gtk_button_set_label gtk_toggle_button_new_with_label \
    gtk_toggle_button_set_active gtk_toggle_button_get_active gtk_toggle_button_set_group \
    gtk_check_button_new gtk_check_button_set_active gtk_check_button_get_active gtk_list_box_new \
    gtk_list_box_append gtk_list_box_remove_all gtk_list_box_set_selection_mode gtk_css_provider_new \
    gtk_css_provider_load_from_string gdk_display_get_default \
    gtk_style_context_add_provider_for_display; do
    only+=(--only "$name")
done

"$vun" util ffi bind "$header" "${flags[@]}" "${scope[@]}" "${libraries[@]}" "${only[@]}" "${closed[@]}" -o todo/gtk/sys.vun
