---
name: munin-prove
description: Show Munin proof and promotion status for the Memory OS read path. Use before trusting promoted memory or release checks.
---

# Munin prove

Run `munin prove --format text`.

Report the gate as green, blocked, or unknown and name missing proof rows. A blocked gate is not automatically a broken product; it means required replay evidence is absent. Do not claim promotion trust without proof rows.
