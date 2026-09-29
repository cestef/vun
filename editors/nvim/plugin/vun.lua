-- Vun in Neovim: its files, their tree-sitter parser and its language server, as the generated
-- `lua/vun/config.lua` describes them. Needs Neovim 0.11; nvim-treesitter builds the parser.
local config = require("vun.config")

vim.filetype.add({ extension = { [config.extension] = config.filetype } })

local ok, parsers = pcall(require, "nvim-treesitter.parsers")
if ok then
  if parsers.get_parser_configs then
    parsers.get_parser_configs()[config.filetype] = { install_info = config.parser, filetype = config.filetype }
  else
    -- nvim-treesitter's rewrite reads parsers when it updates.
    vim.api.nvim_create_autocmd("User", {
      pattern = "TSUpdate",
      callback = function()
        require("nvim-treesitter.parsers")[config.filetype] = { install_info = config.parser }
      end,
    })
  end
end

vim.lsp.config(config.filetype, {
  cmd = vim.list_extend({ config.server.command }, config.server.args),
  filetypes = { config.filetype },
  root_markers = config.root_markers,
})
vim.lsp.enable(config.filetype)
