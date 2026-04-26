local M = {}
local jobs = {}

function M.setup()
  vim.api.nvim_create_user_command("Ghrm", function()
    local file = vim.fn.expand("%:p")
    if jobs[file] then
      vim.notify("ghrm already running for this file", vim.log.levels.WARN)
      return
    end

    local job_id = vim.fn.jobstart({ "ghrm", file }, {
      detach = false,
      on_exit = function()
        jobs[file] = nil
      end,
    })
    jobs[file] = job_id
    vim.notify("Started ghrm for: " .. file, vim.log.levels.INFO)
  end, { desc = "Start ghrm preview for the current file" })

  vim.api.nvim_create_user_command("GhrmStop", function()
    local file = vim.fn.expand("%:p")
    local job_id = jobs[file]
    if job_id then
      vim.fn.jobstop(job_id)
      jobs[file] = nil
      vim.notify("Stopped ghrm for: " .. file, vim.log.levels.INFO)
    else
      vim.notify("No ghrm running for this file", vim.log.levels.WARN)
    end
  end, { desc = "Stop ghrm preview for the current file" })
end

return M
