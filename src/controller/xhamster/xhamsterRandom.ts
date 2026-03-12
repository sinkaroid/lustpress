import { scrapeContent } from "../../scraper/xhamster/xhamsterGetController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError } from "../../utils/modifier";
import { Request, Response } from "express";
import { load } from "cheerio";
import LustPress from "../../LustPress";

const lust = new LustPress();

export async function randomXhamster(req: Request, res: Response) {
  try {
    /**
     * @api {get} /xhamster/random Get random xhamster video
     * @apiName Get random xhamster
     * @apiGroup xhamster
     * @apiDescription Get a random xhamster video from the list of newest videos.
     *
     * @apiSuccessExample {json} Success-Response:
     *   HTTP/1.1 200 OK
     *   HTTP/1.1 400 Bad Request
     *
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/xhamster/random
     *
     * @apiExample {js} JS/TS
     * import axios from "axios"
     *
     * axios.get("https://lust.scathach.id/xhamster/random")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     *
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/xhamster/random") as resp:
     *    print(await resp.json())
     */

    const resolve = await lust.fetchBody(`${c.XHAMSTER}/newest`);
    const $ = load(resolve);

    const videoLinks = $("div.thumb-list__item[data-video-id]")
      .map((_, el) => {
        const href = $(el).find("a[data-role='thumb-link']").attr("href");
        return href && href.includes("/videos/") ? href : null;
      })
      .get()
      .filter(Boolean);

    // Select a random video URL from the list
    const randomIndex = Math.floor(Math.random() * videoLinks.length);
    const randomUrl = videoLinks[randomIndex];

    const data = await scrapeContent(randomUrl);

    logger.info({
      path: req.path,
      query: req.query,
      method: req.method,
      ip: req.ip,
      useragent: req.get("User-Agent"),
    });

    return res.json(data);
  } catch (err) {
    const e = err as Error;
    res.status(400).json(maybeError(false, e.message));
  }
}
