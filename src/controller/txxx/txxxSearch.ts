import { Request, Response } from "express";
import LustPress from "../../LustPress";
import { maybeError } from "../../utils/modifier";

const lust = new LustPress();

export async function searchTxxx(req: Request, res: Response) {
    try {
        const key = String(req.query.key || "").trim();
        const page = Number(req.query.page || 1);

        if (!key) {
            return res.json({
                success: false,
                error: "Parameter key is required",
            });
        }

        if (Number.isNaN(page)) {
            return res.json({
                success: false,
                error: "Parameter page must be a number",
            });
        }

        const apiUrl =
            `https://txxx.com/api/videos2.php` +
            `?params=259200/str/relevance/60/search..${page}.all..` +
            `&s=${encodeURIComponent(key)}`;

        // Fetch from API directly
        const buffer = await lust.fetchBody(apiUrl);
        const rawData = JSON.parse(buffer.toString("utf-8"));

        const videos = Array.isArray(rawData.videos) ? rawData.videos : [];

        const data = videos.map((v: any) => ({
            video_id: v.video_id,
            title: v.title,
            dir: v.dir,
            duration: v.duration,
            views: v.video_viewed,
            rating: v.rating,
            uploaded: v.post_date,
            likes: v.likes,
            dislikes: v.dislikes,
            image: v.scr,
            categories: v.categories ? v.categories.split(",") : [],
            embed: `https://txxx.com/embed/${v.video_id}/`,
        }));

        return res.json({
            success: true,
            total_count: String(rawData.total_count ?? videos.length),
            pages: Number(rawData.pages ?? 1),
            page,
            data,
        });
    } catch (err) {
        const e = err as Error;
        return res.json(maybeError(false, e.message));
    }
}
