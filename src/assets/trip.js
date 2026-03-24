const map = L.map('map');
L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
        attribution: '© OpenStreetMap contributors'
}).addTo(map);

const polyline = L.polyline(TRIP_DATA.polyline, {color: '#e74c3c', weight: 2, opacity: 0.7, lineJoin: 'round'}).addTo(map);
if (TRIP_DATA.gps_len > 0) {
        map.fitBounds(polyline.getBounds().pad(0.1));
} else {
        map.setView([TRIP_DATA.first_lat, TRIP_DATA.first_lon], 6);
}

const markers = {};
TRIP_DATA.markers.forEach(m => {
markers[m.id] = L.marker([m.lat, m.lon], {
        icon: L.divIcon({
                className: '',
                html: '<div class="map-marker"></div>',
                iconSize: [14, 14],
                iconAnchor: [7, 7],
                popupAnchor: [0, 0],
        })
})
        .addTo(map)
        .bindPopup(`<b>${m.location}</b><br><a href="steps/step_${m.id}.html"><img src="${m.thumb}" width="120"><br>Voir l'étape</a>`);
});

Object.values(markers).forEach(marker => {
        let timeout;

        marker.on('mouseover', function () {
                clearTimeout(timeout);
                this.openPopup();
        });
        marker.on('mouseout', function() {
                const popup = this.getPopup().getElement();
                timeout = setTimeout(() => this.closePopup(), 200);

                popup?.addEventListener('mouseenter', () => clearTimeout(timeout));
                popup?.addEventListener('mouseleave', () => this.closePopup());
        });
});

function openMarker(id) {
        const m = markers[id];
        if (!m) return;
        map.panTo(m.getLatLng(), { animate: true, duration: 0.3 });
        m.openPopup();
}
function closeMarker(id) {
        const m = markers[id];
        if (m) m.closePopup();
}
function scrollCarousel(dir) {
        document.getElementById('carousel').scrollBy({ left: dir * 300, behavior: 'smooth' });
}

const observer = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
                if (entry.isIntersecting && entry.intersectionRatio >= 0.6) {
                        const id = entry.target.dataset.stepId;
                        if (!id) return;
                        Object.values(markers).forEach(m => m.closePopup());
                        openMarker(parseInt(id));
                }
        });
}, {
        root: document.getElementById('carousel'),
        threshold: 0.6
});

const carousel = document.getElementById('carousel');
carousel.addEventListener('wheel', (e) => {
        if (e.deltaY === 0) return;

        carousel.scrollBy({ left: event.deltaY, behavior: 'instant' });
        e.preventDefault();
}, { passive: false });

document.querySelectorAll('.card[data-step-id]').forEach(card => {
        observer.observe(card);
});
