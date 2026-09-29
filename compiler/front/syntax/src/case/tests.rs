use super::Case;

#[test]
fn names_are_split_into_words_and_written_in_either_case() {
    let cases = [
        ("point", "Point", "point"),
        ("color_t", "ColorT", "color_t"),
        ("MODE_READ", "ModeRead", "mode_read"),
        ("HAL_UART_Init", "HalUartInit", "hal_uart_init"),
        ("HTTPServer", "HttpServer", "http_server"),
        ("TokenKind", "TokenKind", "token_kind"),
        ("colon_eq", "ColonEq", "colon_eq"),
        ("u8x16", "U8x16", "u8x16"),
        ("_private_ok", "_PrivateOk", "_private_ok"),
    ];
    for (name, pascal, snake) in cases {
        assert_eq!(Case::Pascal.write(name), pascal, "{name}");
        assert_eq!(Case::Snake.write(name), snake, "{name}");
    }
}
